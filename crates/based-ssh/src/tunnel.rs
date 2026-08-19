//! Local-forward an SSH hop to a remote TCP endpoint.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use based_core::SshTunnelConfig;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, load_secret_key};
use russh::{
    Channel, client,
    keys::agent::client::{AgentClient, AgentStream},
    keys::known_hosts::{check_known_hosts, check_known_hosts_path},
};
use tokio::io::{AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, oneshot};

use crate::path::expand_key_path;

struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: Option<PathBuf>,
}

impl client::Handler for ClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let verified = match &self.known_hosts {
            Some(path) => check_known_hosts_path(&self.host, self.port, server_public_key, path),
            None => check_known_hosts(&self.host, self.port, server_public_key),
        };
        match verified {
            Ok(true) => Ok(true),
            Ok(false) => bail!(
                "SSH tunnel: host {} is not in known_hosts. Add it with `ssh {}@{}` once.",
                self.host,
                self.host,
                self.port
            ),
            Err(err) => bail!("SSH tunnel: host key verification failed: {err}"),
        }
    }
}

/// Live local-forward. Dropping it closes the listen port and SSH session.
pub struct SshTunnel {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<Arc<Mutex<client::Handle<ClientHandler>>>>,
}

impl SshTunnel {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            tokio::spawn(async move {
                drop(handle);
            });
        }
    }
}

/// Open a local forward to `remote_host:remote_port` via `ssh`.
pub async fn open_tunnel(
    ssh: &SshTunnelConfig,
    remote_host: &str,
    remote_port: u16,
) -> Result<SshTunnel> {
    open_tunnel_with_known_hosts(ssh, remote_host, remote_port, None).await
}

pub async fn open_tunnel_with_known_hosts(
    ssh: &SshTunnelConfig,
    remote_host: &str,
    remote_port: u16,
    known_hosts: Option<PathBuf>,
) -> Result<SshTunnel> {
    if !ssh.is_configured() {
        bail!("SSH tunnel: host and user are required");
    }

    let handler = ClientHandler {
        host: ssh.host.clone(),
        port: ssh.port,
        known_hosts,
    };
    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, (ssh.host.as_str(), ssh.port), handler)
        .await
        .map_err(|err| anyhow::anyhow!("SSH tunnel: {err}"))?;

    authenticate(&mut session, ssh).await?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("SSH tunnel: bind local port")?;
    let local_addr = listener.local_addr().context("SSH tunnel: local address")?;
    let remote_host = remote_host.to_string();
    let session = Arc::new(Mutex::new(session));
    let handle = session.clone();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    let Ok((socket, _)) = accepted else { break };
                    let handle = handle.clone();
                    let remote_host = remote_host.clone();
                    tokio::spawn(async move {
                        if let Err(err) = forward_one(handle, socket, &remote_host, remote_port).await {
                            log::debug!("SSH tunnel forward ended: {err:#}");
                        }
                    });
                }
            }
        }
    });

    Ok(SshTunnel {
        local_addr,
        shutdown: Some(shutdown_tx),
        handle: Some(session),
    })
}

async fn authenticate(
    session: &mut client::Handle<ClientHandler>,
    ssh: &SshTunnelConfig,
) -> Result<()> {
    if let Some(key_path) = ssh.key_path.as_deref().filter(|p| !p.trim().is_empty()) {
        let path = expand_key_path(key_path);
        let key = load_secret_key(&path, ssh.key_passphrase.as_deref())
            .with_context(|| format!("SSH tunnel: could not load key {}", path.display()))?;
        let hash = session
            .best_supported_rsa_hash()
            .await
            .ok()
            .flatten()
            .flatten();
        let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
        let result = session
            .authenticate_publickey(ssh.user.clone(), key)
            .await
            .context("SSH tunnel: public-key authentication failed")?;
        if !result.success() {
            bail!("SSH tunnel: authentication failed");
        }
        return Ok(());
    }

    authenticate_with_agent(session, &ssh.user).await
}

async fn authenticate_with_agent(
    session: &mut client::Handle<ClientHandler>,
    user: &str,
) -> Result<()> {
    // Keep concrete agent stream types (do not `.dynamic()`): boxing breaks the
    // `'static` bound russh's `Signer` impl needs and surfaces as HRTB errors
    // inside `Tokio::spawn_result` on the Postgres open/test paths.
    #[cfg(unix)]
    {
        let mut agent = AgentClient::connect_env()
            .await
            .context("SSH tunnel: could not connect to ssh-agent (SSH_AUTH_SOCK)")?;
        try_agent_identities(session, user, &mut agent).await
    }
    #[cfg(windows)]
    {
        // `connect_env` is Unix-only (SSH_AUTH_SOCK). Prefer OpenSSH's agent
        // named pipe, then Pageant.
        match AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
            Ok(mut agent) => try_agent_identities(session, user, &mut agent).await,
            Err(openssh_err) => {
                let mut agent = AgentClient::connect_pageant().await.with_context(|| {
                    format!(
                        "SSH tunnel: could not connect to OpenSSH agent \
                             (\\\\.\\pipe\\openssh-ssh-agent: {openssh_err}) or Pageant"
                    )
                })?;
                try_agent_identities(session, user, &mut agent).await
            }
        }
    }
}

async fn try_agent_identities<S>(
    session: &mut client::Handle<ClientHandler>,
    user: &str,
    agent: &mut AgentClient<S>,
) -> Result<()>
where
    // Matches russh's `Signer` impl on `AgentClient<R>` (private `auth` module).
    S: AgentStream + Unpin + Send + 'static,
{
    let identities = agent
        .request_identities()
        .await
        .context("SSH tunnel: ssh-agent has no identities")?;
    if identities.is_empty() {
        bail!("SSH tunnel: ssh-agent has no identities");
    }
    let hash = session
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();
    let hash = hash.or(Some(HashAlg::Sha256));
    let mut last_err = None;
    for public in identities {
        match session
            .authenticate_publickey_with(user, public, hash, agent)
            .await
        {
            Ok(result) if result.success() => return Ok(()),
            Ok(_) => {}
            Err(err) => last_err = Some(err),
        }
    }
    if let Some(err) = last_err {
        bail!("SSH tunnel: authentication failed: {err}");
    }
    bail!("SSH tunnel: authentication failed");
}

async fn forward_one(
    handle: Arc<Mutex<client::Handle<ClientHandler>>>,
    mut socket: TcpStream,
    remote_host: &str,
    remote_port: u16,
) -> Result<()> {
    let channel: Channel<client::Msg> = handle
        .lock()
        .await
        .channel_open_direct_tcpip(remote_host, u32::from(remote_port), "127.0.0.1", 0)
        .await
        .context("SSH tunnel: could not open direct-tcpip channel")?;
    let mut ssh_stream = channel.into_stream();
    let _ = copy_bidirectional(&mut socket, &mut ssh_stream).await;
    let _ = socket.shutdown().await;
    Ok(())
}
