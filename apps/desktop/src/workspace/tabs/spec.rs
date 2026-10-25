use serde::{Deserialize, Serialize};

use crate::connection::ConnectionId;
use crate::workspace::connection_tree::ObjectKind;

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

/// Equality filter a data viewer opens with.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DrillFilter {
    pub column: String,
    pub value: String,
}

/// Identifies what a tab shows. Used by TabManager to open-or-focus.
/// Two specs are equal iff they refer to the same logical panel — prevents duplicate DataViewers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabSpec {
    #[serde(alias = "welcome")]
    Home,
    Dashboard {
        conn_id: ConnectionId,
    },
    DataViewer {
        conn_id: ConnectionId,
        object: String,
        /// Initial `column = value` filter (foreign-key drill-down).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<DrillFilter>,
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
        kind: ObjectKind,
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
    /// Connection-scoped or global panel without a dedicated tab kind (wizards, etc.).
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
            Self::Dashboard { conn_id } => Some(conn_id),
            Self::DataViewer { conn_id, .. } => Some(conn_id),
            Self::QueryEditor { conn_id, .. } => Some(conn_id),
            Self::Pipeline { conn_id, .. } => Some(conn_id),
            Self::Inspector { conn_id, .. } => Some(conn_id),
            Self::ObjectInfo { conn_id, .. } => Some(conn_id),
            Self::DocumentInsert { conn_id, .. } => Some(conn_id),
            Self::Builtin { conn_id, .. } => conn_id.as_ref(),
        }
    }

    /// Whether this tab should be written to / restored from the session snapshot.
    ///
    /// What's New is a one-shot after an in-app update, not workspace state.
    pub fn persist_in_session(&self) -> bool {
        !matches!(self, Self::ReleaseNotes { .. })
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
            filter: None,
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
            filter: None,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn different_objects_not_equal() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
            filter: None,
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "orders".into(),
            filter: None,
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

    #[test]
    fn release_notes_are_not_session_persisted() {
        let notes = TabSpec::ReleaseNotes {
            version: "0.0.0-dev".into(),
        };
        assert!(!notes.persist_in_session());
        assert!(TabSpec::Home.persist_in_session());
    }

    #[test]
    fn dashboard_tab_round_trips_in_session_json() {
        let spec = TabSpec::Dashboard {
            conn_id: ConnectionId("sqlite/local".into()),
        };
        let json = serde_json::to_string(&spec).expect("serialize dashboard tab");
        let restored: TabSpec = serde_json::from_str(&json).expect("deserialize dashboard tab");
        assert_eq!(restored, spec);
    }

    #[test]
    fn existing_session_tab_shapes_still_deserialize() {
        let home: TabSpec = serde_json::from_str(r#"{"type":"home"}"#).unwrap();
        assert_eq!(home, TabSpec::Home);

        let welcome: TabSpec = serde_json::from_str(r#"{"type":"welcome"}"#).unwrap();
        assert_eq!(welcome, TabSpec::Home);

        let viewer: TabSpec =
            serde_json::from_str(r#"{"type":"data_viewer","conn_id":"pg","object":"users"}"#)
                .unwrap();
        assert_eq!(
            viewer,
            TabSpec::DataViewer {
                conn_id: ConnectionId("pg".into()),
                object: "users".into(),
                filter: None,
            }
        );
    }
}
