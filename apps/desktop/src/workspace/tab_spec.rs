use crate::connection::ConnectionId;

/// Identifies what a tab shows. Used by TabManager to open-or-focus.
/// Two specs are equal iff they refer to the same logical panel — prevents duplicate DataViewers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TabSpec {
    Dashboard(ConnectionId),
    DataViewer {
        conn_id: ConnectionId,
        object: String,
    },
    QueryEditor(ConnectionId),
    Pipeline {
        conn_id: ConnectionId,
        collection: String,
    },
    Explain {
        conn_id: ConnectionId,
        label: String,
    },
    Inspector {
        conn_id: ConnectionId,
        object: String,
    },
}

impl TabSpec {
    pub fn conn_id(&self) -> &ConnectionId {
        match self {
            Self::Dashboard(id) => id,
            Self::DataViewer { conn_id, .. } => conn_id,
            Self::QueryEditor(id) => id,
            Self::Pipeline { conn_id, .. } => conn_id,
            Self::Explain { conn_id, .. } => conn_id,
            Self::Inspector { conn_id, .. } => conn_id,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Dashboard(_) => "dashboard",
            Self::DataViewer { .. } => "data viewer",
            Self::QueryEditor(_) => "query",
            Self::Pipeline { .. } => "pipeline",
            Self::Explain { .. } => "explain",
            Self::Inspector { .. } => "structure",
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Dashboard(id) => id.0.clone(),
            Self::DataViewer { object, .. } => object.clone(),
            Self::QueryEditor(_) => "untitled".to_string(),
            Self::Pipeline { collection, .. } => collection.clone(),
            Self::Explain { label, .. } => label.clone(),
            Self::Inspector { object, .. } => object.clone(),
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
        let a = TabSpec::QueryEditor(ConnectionId("pg".into()));
        let b = TabSpec::QueryEditor(ConnectionId("pg".into()));
        assert_eq!(a, b);
    }
}
