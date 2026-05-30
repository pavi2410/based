//! Categorized connection errors for actionable UI (Track A taxonomy).

use std::fmt;

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

/// User-facing connection error with category and optional raw details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionErrorDetail {
    pub category: ConnectionErrorCategory,
    pub details: Option<String>,
}

impl ConnectionErrorDetail {
    pub fn display_message(&self) -> String {
        let mut msg = self.category.summary().to_string();
        if let Some(d) = &self.details
            && !d.is_empty()
        {
            msg.push_str(": ");
            msg.push_str(d);
        }
        msg
    }
}

impl fmt::Display for ConnectionErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_message())
    }
}

/// Best-effort mapping from driver error strings (sqlx, etc.).
pub fn categorize_connect_error(err: &str) -> ConnectionErrorDetail {
    let lower = err.to_ascii_lowercase();
    let category = if lower.contains("password authentication failed")
        || lower.contains("authentication failed")
        || lower.contains("invalid password")
    {
        ConnectionErrorCategory::AuthFailed
    } else if lower.contains("ssl") && lower.contains("required") {
        ConnectionErrorCategory::TlsRequired
    } else if lower.contains("ssl") || lower.contains("tls") {
        ConnectionErrorCategory::TlsRejected
    } else if lower.contains("timed out") || lower.contains("timeout") {
        ConnectionErrorCategory::NetTimeout
    } else if lower.contains("connection refused")
        || lower.contains("could not connect")
        || lower.contains("no route")
        || lower.contains("network is unreachable")
    {
        ConnectionErrorCategory::NetUnreachable
    } else if lower.contains("database") && lower.contains("does not exist") {
        ConnectionErrorCategory::DbNotFound
    } else {
        ConnectionErrorCategory::ServerError
    };
    ConnectionErrorDetail {
        category,
        details: Some(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_auth_failure() {
        let d = categorize_connect_error("password authentication failed for user \"x\"");
        assert_eq!(d.category, ConnectionErrorCategory::AuthFailed);
    }
}
