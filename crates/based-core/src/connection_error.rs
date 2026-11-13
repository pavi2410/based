//! Categorized connection errors for actionable UI (Track A taxonomy).

use std::error::Error;
use std::fmt;
use std::io;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// High-level failure category for connect/test operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectionErrorCategory {
    NetUnreachable,
    NetTimeout,
    AuthFailed,
    AuthMethod,
    TlsRequired,
    TlsRejected,
    DbNotFound,
    ServerError,
    ConfigInvalid,
    UriParse,
    SshFailed,
    Internal,
}

impl ConnectionErrorCategory {
    pub fn summary(self) -> &'static str {
        match self {
            Self::NetUnreachable => "Cannot reach database server",
            Self::NetTimeout => "Connection timed out",
            Self::AuthFailed => "Authentication failed",
            Self::AuthMethod => "Authentication method not supported",
            Self::TlsRequired => "SSL required by server",
            Self::TlsRejected => "SSL handshake failed",
            Self::DbNotFound => "Database does not exist",
            Self::ServerError => "Server rejected connection",
            Self::ConfigInvalid => "Connection settings incomplete",
            Self::UriParse => "Could not parse connection URI",
            Self::SshFailed => "SSH tunnel failed",
            Self::Internal => "Something went wrong",
        }
    }

    pub fn suggested_action(self) -> &'static str {
        match self {
            Self::NetUnreachable => "Check host, port, VPN, and firewall.",
            Self::NetTimeout => "Retry or check server load and network.",
            Self::AuthFailed => "Verify username and password.",
            Self::AuthMethod => "Use a supported authentication method.",
            Self::TlsRequired => "Set SSL mode to Require or higher.",
            Self::TlsRejected => "Check SSL mode and certificates.",
            Self::DbNotFound => "Fix the database name or create the database.",
            Self::ServerError => "See details and check server logs.",
            Self::ConfigInvalid => "Fill all required connection fields.",
            Self::UriParse => "Fix the connection URI format.",
            Self::SshFailed => "Check SSH host, user, and keys.",
            Self::Internal => "Retry; report if this persists.",
        }
    }
}

/// A failed connect or test, classified where it happened (driver error codes and
/// kinds, not message text), with the driver's own message as `detail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectError {
    pub category: ConnectionErrorCategory,
    pub detail: String,
}

impl ConnectError {
    pub fn new(category: ConnectionErrorCategory, detail: impl fmt::Display) -> Self {
        Self {
            category,
            detail: detail.to_string(),
        }
    }

    /// A bug or a crashed task, not something the user can fix in the form.
    pub fn internal(err: impl fmt::Display) -> Self {
        Self::new(ConnectionErrorCategory::Internal, err)
    }

    /// The SSH hop in front of the database failed.
    pub fn ssh(err: impl fmt::Display) -> Self {
        Self::new(ConnectionErrorCategory::SshFailed, err)
    }

    /// The connect attempt ran out of time.
    pub fn timeout(after: Duration) -> Self {
        Self::new(
            ConnectionErrorCategory::NetTimeout,
            format!("no answer after {}s", after.as_secs()),
        )
    }

    /// A socket error, classified by its kind.
    pub fn io(err: &io::Error) -> Self {
        let category = match err.kind() {
            io::ErrorKind::TimedOut => ConnectionErrorCategory::NetTimeout,
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
                ConnectionErrorCategory::ConfigInvalid
            }
            _ => ConnectionErrorCategory::NetUnreachable,
        };
        Self::new(category, err)
    }

    /// Summary, then the driver's message.
    pub fn display_message(&self) -> String {
        if self.detail.is_empty() {
            self.category.summary().to_string()
        } else {
            format!("{}: {}", self.category.summary(), self.detail)
        }
    }
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_message())
    }
}

impl Error for ConnectError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_and_timed_out_sockets() {
        let refused = io::Error::from(io::ErrorKind::ConnectionRefused);
        assert_eq!(
            ConnectError::io(&refused).category,
            ConnectionErrorCategory::NetUnreachable
        );
        let slow = io::Error::from(io::ErrorKind::TimedOut);
        assert_eq!(
            ConnectError::io(&slow).category,
            ConnectionErrorCategory::NetTimeout
        );
    }

    #[test]
    fn message_leads_with_the_category() {
        let e = ConnectError::new(ConnectionErrorCategory::AuthFailed, "bad password");
        assert_eq!(e.to_string(), "Authentication failed: bad password");
    }
}
