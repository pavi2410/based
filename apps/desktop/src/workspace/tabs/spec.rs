use serde::{Deserialize, Serialize};

use based_core::ObjectRef;

use crate::connection::{ConnectionId, EngineKind};
use crate::workspace::connection_tree::ObjectKind;
use crate::workspace::tabs::kind::PanelKind;

/// Typed initialization payload for a query editor tab.
///
/// This replaces the old flat fields on `TabSpec::QueryEditor`
/// so engine-specific init data doesn't pollute the shared type.
/// What a SQL query editor opens with (Postgres, SQLite, Redis console).
///
/// Written with a `"kind": "sql"` tag, so older builds can still read it;
/// Mongo pipelines are [`TabSpec::Pipeline`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename = "sql")]
pub struct QueryEditorInit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(default = "default_auto_run")]
    pub auto_run: bool,
    /// The project query this tab edits, so Save overwrites it after a restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved: Option<SavedQueryRef>,
    /// Unsaved edits at session save time; restored so close still asks first.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dirty: bool,
}

fn default_auto_run() -> bool {
    true
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// A saved project query: its path under `.based/queries/` (no `.toml`) and name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SavedQueryRef {
    pub path: String,
    pub name: String,
}

impl QueryEditorInit {
    /// Whether the tab had unsaved edits when the session was saved.
    pub fn restored_dirty(&self) -> bool {
        self.dirty
    }
}

impl Default for QueryEditorInit {
    fn default() -> Self {
        Self {
            sql: None,
            auto_run: true,
            saved: None,
            dirty: false,
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
        object: ObjectRef,
        /// Initial `column = value` filter (foreign-key drill-down).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<DrillFilter>,
    },
    /// Query/pipeline editor. Always opens a new tab (see TabManager).
    QueryEditor {
        conn_id: ConnectionId,
        init: QueryEditorInit,
    },
    /// MongoDB aggregation pipeline editor.
    Pipeline {
        conn_id: ConnectionId,
        /// `None`: the default collection ([`crate::mongodb::DEFAULT_COLLECTION`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        collection: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pipeline: Option<String>,
    },
    Inspector {
        conn_id: ConnectionId,
        object: ObjectRef,
    },
    /// Lightweight placeholder for triggers and other objects without a schema inspector tab.
    ObjectInfo {
        conn_id: ConnectionId,
        #[serde(alias = "object_name")]
        object: ObjectRef,
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
    /// The connection form, new or editing `editing`. Not saved: an empty form
    /// isn't worth restoring. Opened by `open_connection_wizard`, not dispatch.
    #[serde(skip)]
    ConnectionWizard {
        editing: Option<ConnectionId>,
    },
    /// A live panel with no restorable spec (a connection's default change
    /// stream or console, recreated whenever the connection opens). Not saved.
    #[serde(skip)]
    Transient {
        conn_id: Option<ConnectionId>,
        kind: PanelKind,
    },
}

impl TabSpec {
    /// Sessions written before objects were structured stored Postgres objects as
    /// `"schema.name"`, which now reads as a bare name. Split those the way they
    /// were opened then (last dot, else `public`) so a restored tab equals the one
    /// the tree opens. Other engines never had a schema, so their names stay whole.
    pub fn migrate_legacy_object(&mut self, engine: EngineKind) {
        if engine != EngineKind::Postgres {
            return;
        }
        if let Self::DataViewer { object, .. }
        | Self::Inspector { object, .. }
        | Self::ObjectInfo { object, .. } = self
            && object.schema.is_none()
        {
            *object = ObjectRef::from_legacy_qualified(&object.name, "public");
        }
    }

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
            Self::ConnectionWizard { editing } => editing.as_ref(),
            Self::Transient { conn_id, .. } => conn_id.as_ref(),
        }
    }

    /// Whether this tab should be written to / restored from the session snapshot.
    ///
    /// What's New is a one-shot after an in-app update, not workspace state; the
    /// wizard and transient panels have nothing to restore.
    pub fn persist_in_session(&self) -> bool {
        !matches!(
            self,
            Self::ReleaseNotes { .. } | Self::ConnectionWizard { .. } | Self::Transient { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_viewer_equality() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("pg"),
            object: ObjectRef::bare("users"),
            filter: None,
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("pg"),
            object: ObjectRef::bare("users"),
            filter: None,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn different_objects_not_equal() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("pg"),
            object: ObjectRef::bare("users"),
            filter: None,
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("pg"),
            object: ObjectRef::bare("orders"),
            filter: None,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn query_editor_specs_equal_but_open_or_focus_creates_distinct_tabs() {
        // Spec equality treats two QueryEditors the same conn as equal; TabManager::open_or_focus
        // still opens a fresh tab whenever the caller passes TabSpec::QueryEditor (branch `is_query`).
        let a = TabSpec::blank_query_editor(ConnectionId::parse("pg"));
        let b = TabSpec::blank_query_editor(ConnectionId::parse("pg"));
        assert_eq!(a, b);
    }

    #[test]
    fn query_editor_init_reads_sessions_saved_before_saved_and_dirty() {
        let old = r#"{"kind":"sql","sql":"select 1","auto_run":false}"#;
        let init: QueryEditorInit = serde_json::from_str(old).unwrap();
        assert_eq!(
            init,
            QueryEditorInit {
                sql: Some("select 1".into()),
                auto_run: false,
                saved: None,
                dirty: false,
            }
        );
        assert!(!init.restored_dirty());
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
            conn_id: ConnectionId::parse("sqlite/local"),
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
                conn_id: ConnectionId::parse("pg"),
                object: ObjectRef::bare("users"),
                filter: None,
            }
        );
    }

    #[test]
    fn legacy_postgres_objects_migrate_to_what_the_tree_opens() {
        let legacy = r#"[
            {"type":"data_viewer","conn_id":"pg","object":"public.users"},
            {"type":"inspector","conn_id":"pg","object":"sales.orders"},
            {"type":"object_info","conn_id":"pg","object_name":"audit.trg","kind":"trigger"}
        ]"#;
        let mut specs: Vec<TabSpec> = serde_json::from_str(legacy).unwrap();
        for s in &mut specs {
            s.migrate_legacy_object(EngineKind::Postgres);
        }
        let pg = ConnectionId::parse("pg");
        assert_eq!(
            specs,
            [
                TabSpec::DataViewer {
                    conn_id: pg.clone(),
                    object: ObjectRef::qualified("public", "users"),
                    filter: None,
                },
                TabSpec::Inspector {
                    conn_id: pg.clone(),
                    object: ObjectRef::qualified("sales", "orders"),
                },
                TabSpec::ObjectInfo {
                    conn_id: pg,
                    object: ObjectRef::qualified("audit", "trg"),
                    kind: ObjectKind::Trigger,
                },
            ]
        );
    }

    #[test]
    fn legacy_names_of_other_engines_stay_whole() {
        let mut spec: TabSpec = serde_json::from_str(
            r#"{"type":"data_viewer","conn_id":"m","object":"system.profile"}"#,
        )
        .unwrap();
        spec.migrate_legacy_object(EngineKind::MongoDB);
        assert!(matches!(
            spec,
            TabSpec::DataViewer { object, .. } if object == ObjectRef::bare("system.profile")
        ));
    }
}
