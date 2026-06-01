//! Authentication method types.
//!
//! [`AuthMethod`] is the canonical representation of how Based authenticates
//! to a database endpoint. It is intentionally separate from engine-specific
//! connection configs so that auth concerns (SSH tunneling, IAM, vault) can
//! evolve without changing the per-engine config schemas.
//!
//! # Migration path
//!
//! Today, auth is embedded in each engine's config struct (e.g. `PostgresConfig`
//! has password/SSL fields). The planned migration is:
//! 1. Add `auth: AuthMethod` to `ConnectionConfig` (with `#[serde(default)]`)
//! 2. Move per-engine auth fields into `AuthMethod` variants
//! 3. Bump `.based/` `schema_version` and add a migration in `based-project`
//!
//! Until step 1 lands, `AuthMethod` is referenced by new code only (CLI flag
//! parsing, future connection wizard fields, CI usage).

use serde::{Deserialize, Serialize};

/// How Based authenticates to a database endpoint.
///
/// Variants are additive — existing configs without an `auth` field deserialize
/// as [`AuthMethod::default()`] (password auth with an empty username, resolved
/// from `.env` at connect time).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// Username + password. Password is resolved from `.env` or a secrets manager at connect time.
    Password { username: String },

    /// SSH jump host that tunnels to a database endpoint.
    ///
    /// The `inner` auth method applies to the database behind the tunnel;
    /// the SSH connection itself uses the host/username/key fields.
    SshTunnel {
        host: String,
        port: u16,
        username: String,
        /// Path to an SSH private key file. `None` delegates to the SSH agent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key_path: Option<String>,
        /// Auth for the database endpoint behind the tunnel.
        inner: Box<AuthMethod>,
    },

    /// AWS IAM database authentication (RDS IAM tokens, DocumentDB, Atlas).
    AwsIam {
        region: String,
        /// AWS named profile. `None` uses the default credential chain.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },

    /// Mutual TLS — client certificate + private key.
    ClientCertificate { cert_path: String, key_path: String },

    /// Externally-managed token (OAuth2, OIDC, Vault, etc.).
    ///
    /// The `provider` string is an opaque identifier resolved by a credentials
    /// plugin at runtime. Future: plugin registry in based-core.
    External { provider: String },
}

impl Default for AuthMethod {
    /// Default to password auth; username is resolved from `.env` at connect time.
    fn default() -> Self {
        Self::Password {
            username: String::new(),
        }
    }
}

impl AuthMethod {
    /// Returns `true` if this auth method involves a network hop before the database
    /// (e.g. SSH tunnel). Used to annotate UI indicators and telemetry.
    pub fn has_tunnel(&self) -> bool {
        matches!(self, Self::SshTunnel { .. })
    }

    /// Returns the innermost [`AuthMethod`] by unwrapping nested SSH tunnels.
    pub fn inner_auth(&self) -> &AuthMethod {
        match self {
            Self::SshTunnel { inner, .. } => inner.inner_auth(),
            other => other,
        }
    }

    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Password { .. } => "Password",
            Self::SshTunnel { .. } => "SSH Tunnel",
            Self::AwsIam { .. } => "AWS IAM",
            Self::ClientCertificate { .. } => "Client Certificate",
            Self::External { .. } => "External / OAuth",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_password() {
        assert!(matches!(AuthMethod::default(), AuthMethod::Password { .. }));
    }

    #[test]
    fn ssh_tunnel_has_tunnel() {
        let m = AuthMethod::SshTunnel {
            host: "bastion.example.com".into(),
            port: 22,
            username: "ec2-user".into(),
            key_path: None,
            inner: Box::new(AuthMethod::Password {
                username: "admin".into(),
            }),
        };
        assert!(m.has_tunnel());
        assert!(!m.inner_auth().has_tunnel());
    }

    #[test]
    fn inner_auth_unwraps_nested_tunnels() {
        let inner = AuthMethod::AwsIam {
            region: "us-east-1".into(),
            profile: None,
        };
        let outer = AuthMethod::SshTunnel {
            host: "host".into(),
            port: 22,
            username: "u".into(),
            key_path: None,
            inner: Box::new(inner.clone()),
        };
        assert_eq!(outer.inner_auth(), &inner);
    }

    #[test]
    fn serde_round_trip_password() {
        let m = AuthMethod::Password {
            username: "alice".into(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: AuthMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn serde_round_trip_ssh_tunnel() {
        let m = AuthMethod::SshTunnel {
            host: "bastion.host.com".into(),
            port: 22,
            username: "deploy".into(),
            key_path: Some("/home/deploy/.ssh/id_rsa".into()),
            inner: Box::new(AuthMethod::AwsIam {
                region: "eu-west-1".into(),
                profile: Some("prod".into()),
            }),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: AuthMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn label_strings() {
        assert_eq!(AuthMethod::default().label(), "Password");
        assert_eq!(
            AuthMethod::ClientCertificate {
                cert_path: "a".into(),
                key_path: "b".into()
            }
            .label(),
            "Client Certificate"
        );
    }
}
