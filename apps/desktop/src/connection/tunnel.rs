//! Optional SSH hop in front of a TCP database endpoint.

use anyhow::{Result, bail};
use based_core::SshTunnelConfig;
use based_ssh::{SshTunnel, open_tunnel};

use crate::postgres::SslMode;

pub fn ssh_hostname_verify_supported(ssl_mode: SslMode) -> Result<()> {
    match ssl_mode {
        SslMode::VerifyCa | SslMode::VerifyFull => {
            bail!("SSH tunnel: hostname verify is not supported through SSH yet; use SSL require")
        }
        _ => Ok(()),
    }
}

pub async fn open_optional_tunnel(
    ssh: Option<&SshTunnelConfig>,
    remote_host: &str,
    remote_port: u16,
    ssl_mode: SslMode,
) -> Result<Option<SshTunnel>> {
    let Some(ssh) = ssh else {
        return Ok(None);
    };
    ssh_hostname_verify_supported(ssl_mode)?;
    Ok(Some(open_tunnel(ssh, remote_host, remote_port).await?))
}

pub fn rewrite_tcp_endpoint(host: &mut String, port: &mut u16, tunnel: &SshTunnel) {
    *host = "127.0.0.1".into();
    *port = tunnel.local_port();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_and_disable_are_supported() {
        ssh_hostname_verify_supported(SslMode::Disable).unwrap();
        ssh_hostname_verify_supported(SslMode::Prefer).unwrap();
        ssh_hostname_verify_supported(SslMode::Require).unwrap();
    }

    #[test]
    fn verify_modes_are_rejected() {
        let err = ssh_hostname_verify_supported(SslMode::VerifyFull).unwrap_err();
        assert!(err.to_string().contains("hostname verify"));
        assert!(ssh_hostname_verify_supported(SslMode::VerifyCa).is_err());
    }
}
