//! Closed set of dock panel kinds. Every panel takes its `panel_name` from
//! [`names`], and [`PanelKind::from_name`] matches those exact strings, so a
//! renamed or new panel can't be classified by accident.

use std::sync::Arc;

use gpui_kit::App;
use gpui_kit::component::dock::PanelView;

/// Dock panel names (`Panel::panel_name`). Also stored in the dock layout.
pub mod names {
    pub const HOME: &str = "HomePanel";
    pub const DASHBOARD: &str = "ConnectionDashboard";
    pub const WIZARD: &str = "ConnectionWizard";
    pub const RELEASE_NOTES: &str = "ReleaseNotesPanel";
    pub const OBJECT_INFO: &str = "ObjectInfo";
    pub const PG_QUERY_EDITOR: &str = "PgQueryEditor";
    pub const SQLITE_QUERY_EDITOR: &str = "SqliteQueryEditor";
    pub const PG_DATA_VIEWER: &str = "PgDataViewer";
    pub const SQLITE_DATA_VIEWER: &str = "SqliteDataViewer";
    pub const PG_TABLE_INSPECTOR: &str = "PgTableInspector";
    pub const SQLITE_TABLE_INSPECTOR: &str = "SqliteTableInspector";
    pub const MONGO_PIPELINE: &str = "MongoPipelineBuilder";
    pub const MONGO_INSPECTOR: &str = "MongoCollectionInspector";
    pub const MONGO_DOCUMENT_VIEWER: &str = "MongoDocumentViewer";
    pub const MONGO_DOCUMENT_EDITOR: &str = "MongoDocumentEditor";
    pub const MONGO_CHANGE_STREAM: &str = "MongoChangeStream";
    pub const REDIS_CONSOLE: &str = "RedisConsole";
    pub const REDIS_KEY_VIEW: &str = "RedisKeyView";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Home,
    Dashboard,
    Wizard,
    ReleaseNotes,
    ObjectInfo,
    QueryEditor,
    Pipeline,
    DataViewer,
    Inspector,
    DocumentViewer,
    DocumentEditor,
    ChangeStream,
    RedisConsole,
    Redis,
    Other,
}

impl PanelKind {
    pub fn from_name(name: &str) -> Self {
        use names::*;
        match name {
            HOME => Self::Home,
            DASHBOARD => Self::Dashboard,
            WIZARD => Self::Wizard,
            RELEASE_NOTES => Self::ReleaseNotes,
            OBJECT_INFO => Self::ObjectInfo,
            PG_QUERY_EDITOR | SQLITE_QUERY_EDITOR => Self::QueryEditor,
            MONGO_PIPELINE => Self::Pipeline,
            PG_DATA_VIEWER | SQLITE_DATA_VIEWER => Self::DataViewer,
            PG_TABLE_INSPECTOR | SQLITE_TABLE_INSPECTOR | MONGO_INSPECTOR => Self::Inspector,
            MONGO_DOCUMENT_VIEWER => Self::DocumentViewer,
            MONGO_DOCUMENT_EDITOR => Self::DocumentEditor,
            MONGO_CHANGE_STREAM => Self::ChangeStream,
            REDIS_CONSOLE => Self::RedisConsole,
            REDIS_KEY_VIEW => Self::Redis,
            _ => Self::Other,
        }
    }

    /// Fallback tab title for a panel without a spec of its own.
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Dashboard => "Connection",
            Self::Wizard => "New connection",
            Self::ReleaseNotes => "What's New",
            Self::ObjectInfo => "Object",
            Self::QueryEditor => "Query",
            Self::Pipeline => "Pipeline",
            Self::DataViewer => "Data",
            Self::Inspector => "Schema",
            Self::DocumentViewer => "Documents",
            Self::DocumentEditor => "Document",
            Self::ChangeStream => "Change stream",
            Self::RedisConsole => "Console",
            Self::Redis => "Key",
            Self::Other => "Panel",
        }
    }

    pub fn of(panel: &Arc<dyn PanelView>, cx: &App) -> Self {
        Self::from_name(panel.panel_name(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_exact_names_only() {
        assert_eq!(
            PanelKind::from_name(names::PG_DATA_VIEWER),
            PanelKind::DataViewer
        );
        assert_eq!(
            PanelKind::from_name(names::MONGO_INSPECTOR),
            PanelKind::Inspector
        );
        assert_eq!(
            PanelKind::from_name(names::REDIS_KEY_VIEW),
            PanelKind::Redis
        );
        assert_eq!(PanelKind::from_name("MysqlDataViewer"), PanelKind::Other);
        assert_eq!(PanelKind::from_name("Mystery"), PanelKind::Other);
    }
}
