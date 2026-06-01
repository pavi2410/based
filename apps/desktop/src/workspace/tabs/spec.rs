use serde::{Deserialize, Serialize};

use crate::connection::ConnectionId;

/// Classifies a tab by the number and kind of connections it operates on.
///
/// Use this to decide whether a tab survives connection disconnect, appears
/// in connection-scoped menus, or is surfaced in workspace-level views
/// (e.g. ER diagrams, agent chat, chart builder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabScope {
    /// Tab operates on exactly one connection.
    Connection(ConnectionId),
    /// Tab spans multiple connections (ER diagrams, cross-DB joins).
    MultiConnection(Vec<ConnectionId>),
    /// Workspace-scoped but not connection-specific (agent chat, chart builder).
    Workspace,
    /// Fully global — not tied to any project or connection (Home, ReleaseNotes).
    Global,
}

/// Typed initialization payload for a query editor tab.
///
/// This replaces the old flat fields on `TabSpec::QueryEditor`
/// so engine-specific init data doesn't pollute the shared type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryEditorInit {
    /// SQL editor — Postgres or SQLite.
    Sql {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sql: Option<String>,
        #[serde(default = "default_auto_run")]
        auto_run: bool,
    },
    /// MongoDB aggregation pipeline editor.
    MongoPipeline {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pipeline: Option<String>,
        /// Target collection name. `None` means use a default collection name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        collection: Option<String>,
    },
}

fn default_auto_run() -> bool {
    true
}

impl Default for QueryEditorInit {
    fn default() -> Self {
        Self::Sql {
            sql: None,
            auto_run: true,
        }
    }
}

/// Identifies what a tab shows. Used by TabManager to open-or-focus.
/// Two specs are equal iff they refer to the same logical panel — prevents duplicate DataViewers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabSpec {
    #[serde(alias = "welcome")]
    Home,
    Dashboard(ConnectionId),
    DataViewer {
        conn_id: ConnectionId,
        object: String,
    },
    /// Query/pipeline editor. Always opens a new tab (see TabManager).
    QueryEditor {
        conn_id: ConnectionId,
        init: QueryEditorInit,
    },
    Pipeline {
        conn_id: ConnectionId,
        collection: String,
    },
    Inspector {
        conn_id: ConnectionId,
        object: String,
    },
    /// Lightweight placeholder for triggers and other objects without a schema inspector tab.
    ObjectInfo {
        conn_id: ConnectionId,
        object_name: String,
        kind_label: String,
    },
    /// MongoDB insert-document JSON editor for a collection.
    DocumentInsert {
        conn_id: ConnectionId,
        collection: String,
    },
    /// Release notes for a specific app version (fetched async in panel).
    ReleaseNotes {
        version: String,
    },
    /// Connection-scoped or global panel without a dedicated tab kind (PRAGMA browser, wizards, etc.).
    Builtin {
        conn_id: Option<ConnectionId>,
        panel: String,
    },
}

impl TabSpec {
    pub fn blank_query_editor(conn_id: ConnectionId) -> Self {
        Self::QueryEditor {
            conn_id,
            init: QueryEditorInit::default(),
        }
    }

    /// Returns the primary connection ID for this tab, or `None` for global/workspace tabs.
    ///
    /// Use [`TabSpec::scope()`] for richer classification.
    pub fn conn_id(&self) -> Option<&ConnectionId> {
        match self {
            Self::Home | Self::ReleaseNotes { .. } => None,
            Self::Dashboard(id) => Some(id),
            Self::DataViewer { conn_id, .. } => Some(conn_id),
            Self::QueryEditor { conn_id, .. } => Some(conn_id),
            Self::Pipeline { conn_id, .. } => Some(conn_id),
            Self::Inspector { conn_id, .. } => Some(conn_id),
            Self::ObjectInfo { conn_id, .. } => Some(conn_id),
            Self::DocumentInsert { conn_id, .. } => Some(conn_id),
            Self::Builtin { conn_id, .. } => conn_id.as_ref(),
        }
    }

    /// Returns the connection scope for this tab.
    pub fn scope(&self) -> TabScope {
        match self {
            Self::Home | Self::ReleaseNotes { .. } => TabScope::Global,
            Self::Dashboard(id) => TabScope::Connection(id.clone()),
            Self::DataViewer { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::QueryEditor { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Pipeline { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Inspector { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::ObjectInfo { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::DocumentInsert { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Builtin { conn_id, .. } => conn_id
                .as_ref()
                .cloned()
                .map(TabScope::Connection)
                .unwrap_or(TabScope::Global),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Dashboard(_) => "dashboard",
            Self::DataViewer { .. } => "data viewer",
            Self::QueryEditor { .. } => "query",
            Self::Pipeline { .. } => "pipeline",
            Self::Inspector { .. } => "structure",
            Self::ObjectInfo { .. } => "object",
            Self::DocumentInsert { .. } => "insert",
            Self::ReleaseNotes { .. } => "release notes",
            Self::Builtin { .. } => "panel",
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Home => "Home".to_string(),
            Self::Dashboard(id) => id.0.clone(),
            Self::DataViewer { object, .. } => object.clone(),
            Self::QueryEditor { .. } => "untitled".to_string(),
            Self::Pipeline { collection, .. } => collection.clone(),
            Self::Inspector { object, .. } => object.clone(),
            Self::ObjectInfo { object_name, .. } => object_name.clone(),
            Self::DocumentInsert { collection, .. } => format!("Insert · {collection}"),
            Self::ReleaseNotes { version } => format!("What's New in v{version}"),
            Self::Builtin { panel, .. } => panel.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_viewer_equality() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn different_objects_not_equal() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "orders".into(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn query_editor_specs_equal_but_open_or_focus_creates_distinct_tabs() {
        // Spec equality treats two QueryEditors the same conn as equal; TabManager::open_or_focus
        // still opens a fresh tab whenever the caller passes TabSpec::QueryEditor (branch `is_query`).
        let a = TabSpec::blank_query_editor(ConnectionId("pg".into()));
        let b = TabSpec::blank_query_editor(ConnectionId("pg".into()));
        assert_eq!(a, b);
    }
}
