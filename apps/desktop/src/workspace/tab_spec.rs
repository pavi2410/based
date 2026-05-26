use serde::{Deserialize, Serialize};

use crate::connection::ConnectionId;

/// Identifies what a tab shows. Used by TabManager to open-or-focus.
/// Two specs are equal iff they refer to the same logical panel — prevents duplicate DataViewers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabSpec {
    Dashboard(ConnectionId),
    DataViewer {
        conn_id: ConnectionId,
        object: String,
    },
    /// SQL editors (Postgres/SQLite) or Mongo pipeline workspace (initial_pipeline).
    QueryEditor {
        conn_id: ConnectionId,
        initial_sql: Option<String>,
        initial_pipeline: Option<String>,
        auto_run: bool,
        /// Target Mongo collection when opening a pipeline/query tab (PG/SQLite ignore).
        mongo_collection: Option<String>,
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
}

impl TabSpec {
    pub fn blank_query_editor(conn_id: ConnectionId) -> Self {
        Self::QueryEditor {
            conn_id,
            initial_sql: None,
            initial_pipeline: None,
            auto_run: true,
            mongo_collection: None,
        }
    }

    pub fn conn_id(&self) -> &ConnectionId {
        match self {
            Self::Dashboard(id) => id,
            Self::DataViewer { conn_id, .. } => conn_id,
            Self::QueryEditor { conn_id, .. } => conn_id,
            Self::Pipeline { conn_id, .. } => conn_id,
            Self::Inspector { conn_id, .. } => conn_id,
            Self::ObjectInfo { conn_id, .. } => conn_id,
            Self::DocumentInsert { conn_id, .. } => conn_id,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Dashboard(_) => "dashboard",
            Self::DataViewer { .. } => "data viewer",
            Self::QueryEditor { .. } => "query",
            Self::Pipeline { .. } => "pipeline",
            Self::Inspector { .. } => "structure",
            Self::ObjectInfo { .. } => "object",
            Self::DocumentInsert { .. } => "insert",
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Dashboard(id) => id.0.clone(),
            Self::DataViewer { object, .. } => object.clone(),
            Self::QueryEditor { .. } => "untitled".to_string(),
            Self::Pipeline { collection, .. } => collection.clone(),
            Self::Inspector { object, .. } => object.clone(),
            Self::ObjectInfo { object_name, .. } => object_name.clone(),
            Self::DocumentInsert { collection, .. } => format!("Insert · {collection}"),
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
