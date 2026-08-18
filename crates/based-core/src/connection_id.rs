use std::fmt::{Display, Formatter, Result};

use serde::{Deserialize, Serialize};

/// Stable opaque identifier for a connection profile or config key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionId(pub String);

impl ConnectionId {
    /// Prefix for user-local workspace wizard templates (not `.based/connections/`).
    pub const WORKSPACE_TEMPLATE_PREFIX: &'static str = "ws-template:";
    /// Prefix for connections stored in the personal based-dir (`~/.config/based`).
    pub const USER_PREFIX: &'static str = "user:";
    /// Prefix for a live session that has not been saved to a based-dir.
    pub const UNSAVED_PREFIX: &'static str = "unsaved:";

    pub fn from_key(key: &str) -> Self {
        Self(key.to_string())
    }

    /// Personal tree id: `user:` + relative path id (`user:local/northwind`).
    pub fn personal(relative_id: &str) -> Self {
        Self(format!("{}{relative_id}", Self::USER_PREFIX))
    }

    /// Live wizard session that is not written to disk: `unsaved:` + key.
    pub fn unsaved(key: &str) -> Self {
        Self(format!("{}{key}", Self::UNSAVED_PREFIX))
    }

    /// User-local connection stored in workspace metadata, not the open `.based/` project.
    pub fn is_workspace_local(&self) -> bool {
        self.is_personal() || self.is_ephemeral()
    }

    /// Connection loaded from the personal based-dir.
    pub fn is_personal(&self) -> bool {
        self.0.starts_with(Self::USER_PREFIX)
    }

    /// Live session that is not a project or personal file.
    pub fn is_unsaved(&self) -> bool {
        self.0.starts_with(Self::UNSAVED_PREFIX)
    }

    /// Unsaved live sessions and workspace SQLite templates — never owned by a based-dir snapshot.
    pub fn is_ephemeral(&self) -> bool {
        self.is_unsaved() || self.0.starts_with(Self::WORKSPACE_TEMPLATE_PREFIX)
    }
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_template_ids_are_workspace_local() {
        let id = ConnectionId::from_key("ws-template:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert!(id.is_workspace_local());
    }

    #[test]
    fn based_connection_ids_are_project_owned() {
        let id = ConnectionId::from_key("local/northwind");
        assert!(!id.is_workspace_local());
        assert!(!id.is_personal());
    }

    #[test]
    fn personal_ids_use_user_prefix_and_are_workspace_local() {
        let id = ConnectionId::personal("local/northwind");
        assert_eq!(id.0, "user:local/northwind");
        assert!(id.is_personal());
        assert!(id.is_workspace_local());
        assert_ne!(id, ConnectionId::from_key("local/northwind"));
    }

    #[test]
    fn unsaved_ids_are_ephemeral_and_workspace_local() {
        let id = ConnectionId::unsaved("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(id.0, "unsaved:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert!(id.is_unsaved());
        assert!(id.is_ephemeral());
        assert!(id.is_workspace_local());
        assert!(!id.is_personal());
    }

    #[test]
    fn project_and_personal_ids_are_not_ephemeral() {
        assert!(!ConnectionId::from_key("local/northwind").is_ephemeral());
        assert!(!ConnectionId::personal("analytics").is_ephemeral());
        assert!(!ConnectionId::personal("analytics").is_unsaved());
    }
}
