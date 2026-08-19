//! Runtime SSH tunnel settings (resolved secrets, no project-file types).

use serde::{Deserialize, Serialize};

fn default_ssh_port() -> u16 {
    22
}

/// One SSH hop used as a local-forward transport to a database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTunnelConfig {
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_passphrase: Option<String>,
}

impl SshTunnelConfig {
    pub fn is_configured(&self) -> bool {
        !self.host.trim().is_empty() && !self.user.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_22() {
        let m: SshTunnelConfig =
            serde_json::from_str(r#"{"host":"bastion.example.com","user":"ec2-user"}"#).unwrap();
        assert_eq!(m.port, 22);
        assert!(m.key_path.is_none());
        assert!(m.is_configured());
    }

    #[test]
    fn empty_host_or_user_is_not_configured() {
        let m = SshTunnelConfig {
            host: "  ".into(),
            port: 22,
            user: "ec2-user".into(),
            key_path: None,
            key_passphrase: None,
        };
        assert!(!m.is_configured());
    }
}
