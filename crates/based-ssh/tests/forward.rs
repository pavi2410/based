use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use based_core::SshTunnelConfig;
use based_ssh::open_tunnel_with_known_hosts;
use rand_core::OsRng;
use russh::keys::known_hosts::learn_known_hosts_path;
use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, PrivateKey, PublicKey};
use russh::server::{Auth, Config, Msg, Server as _, Session};
use russh::{Channel, server};
use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct ForwardServer {
    client_pubkey: PublicKey,
}

impl server::Server for ForwardServer {
    type Handler = Handler;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        Handler {
            client_pubkey: self.client_pubkey.clone(),
        }
    }
}

struct Handler {
    client_pubkey: PublicKey,
}

impl server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        if public_key == &self.client_pubkey {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let mut dest = TcpStream::connect((host_to_connect, port_to_connect as u16))
            .await
            .map_err(|_| russh::Error::Disconnect)?;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = copy_bidirectional(&mut dest, &mut stream).await;
        });
        Ok(true)
    }
}

async fn start_echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 64];
        let n = socket.read(&mut buf).await.unwrap();
        socket.write_all(&buf[..n]).await.unwrap();
    });
    port
}

async fn start_ssh(server_key: PrivateKey, client_pubkey: PublicKey) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        auth_rejection_time: Duration::from_millis(1),
        auth_rejection_time_initial: Some(Duration::ZERO),
        keys: vec![server_key],
        ..Default::default()
    };
    let mut server = ForwardServer { client_pubkey };
    tokio::spawn(async move {
        let _ = server.run_on_socket(Arc::new(config), &listener).await;
    });
    port
}

#[tokio::test]
async fn unknown_host_key_is_rejected() {
    let client_key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
    let server_key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
    let ssh_port = start_ssh(server_key, client_key.public_key().clone()).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    client_key
        .write_openssh_file(&key_path, LineEnding::LF)
        .unwrap();
    let known_hosts = dir.path().join("known_hosts");
    fs::write(&known_hosts, "").unwrap();

    let ssh = SshTunnelConfig {
        host: "127.0.0.1".into(),
        port: ssh_port,
        user: "test".into(),
        key_path: Some(key_path.display().to_string()),
        key_passphrase: None,
    };
    let err = match open_tunnel_with_known_hosts(&ssh, "127.0.0.1", 1, Some(known_hosts)).await {
        Ok(_) => panic!("expected unknown host key to fail"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("known_hosts") || msg.contains("SSH tunnel"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn forwards_bytes_through_local_port() {
    let echo_port = start_echo().await;
    let client_key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
    let server_key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
    let server_pub = server_key.public_key().clone();
    let ssh_port = start_ssh(server_key, client_key.public_key().clone()).await;

    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    client_key
        .write_openssh_file(&key_path, LineEnding::LF)
        .unwrap();
    let known_hosts = dir.path().join("known_hosts");
    learn_known_hosts_path("127.0.0.1", ssh_port, &server_pub, &known_hosts).unwrap();

    let ssh = SshTunnelConfig {
        host: "127.0.0.1".into(),
        port: ssh_port,
        user: "test".into(),
        key_path: Some(key_path.display().to_string()),
        key_passphrase: None,
    };
    let tunnel = open_tunnel_with_known_hosts(&ssh, "127.0.0.1", echo_port, Some(known_hosts))
        .await
        .expect("open tunnel");

    let mut client = TcpStream::connect(tunnel.local_addr()).await.unwrap();
    client.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    client.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");
}
