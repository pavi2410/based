//! Canonical identifiers for everything the UI addresses: a project, a
//! connection within that project, and an individual tab/workspace item.
//!
//! Having one type prevents the current tuple-passing pattern
//! (`(project_path, conn_key, ...)`) from leaking through every command,
//! state map key, and React Query cache key, which is the main source of
//! drift between the Rust and TS sides.
//!
//! The constructors and accessors are allowed to be unused during Phase 0
//! — they are the canonical surface Phases 1-2 will migrate onto.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::connection_id::ConnectionId;

/// Identifies a project on disk. Currently its absolute filesystem path;
/// wrapped so we can swap in a stable ID later without touching callers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
pub struct ProjectAddress(pub String);

impl ProjectAddress {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectAddress {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ProjectAddress {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Identifies a specific connection inside a project (by the user-visible
/// key from `.based/config.toml`, e.g. `"dev"` or `"prod"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct ConnectionAddress {
    pub project: ProjectAddress,
    pub conn_key: String,
}

impl ConnectionAddress {
    pub fn new(project: impl Into<ProjectAddress>, conn_key: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            conn_key: conn_key.into(),
        }
    }

    /// Stable hash-based connection ID used by the registry.
    pub fn id(&self) -> ConnectionId {
        ConnectionId::new(self.project.as_str(), &self.conn_key)
    }
}

/// Identifies a workspace tab. The UI can address query tabs, table browse
/// tabs, and pinned inspector tabs uniformly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TabAddress {
    /// A SQL / MongoDB query tab. `id` is a UI-owned identifier (e.g. nanoid).
    Query {
        connection: ConnectionAddress,
        id: String,
    },
    /// A table/collection browse tab.
    Table {
        connection: ConnectionAddress,
        schema: Option<String>,
        name: String,
    },
    /// A schema-inspector tab for a single object.
    Inspector {
        connection: ConnectionAddress,
        schema: Option<String>,
        name: String,
    },
}
