//! Closed set of dock panel kinds, classified once from `Panel::panel_name`.

use std::sync::Arc;

use gpui_kit::App;
use gpui_kit::component::dock::PanelView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        match name {
            "HomePanel" => Self::Home,
            "ConnectionDashboard" => Self::Dashboard,
            "ConnectionWizard" => Self::Wizard,
            "ReleaseNotesPanel" => Self::ReleaseNotes,
            "ObjectInfo" => Self::ObjectInfo,
            "PgQueryEditor" | "SqliteQueryEditor" => Self::QueryEditor,
            "MongoPipelineBuilder" => Self::Pipeline,
            n if n.ends_with("DataViewer") => Self::DataViewer,
            n if n.ends_with("Inspector") => Self::Inspector,
            n if n.ends_with("DocumentViewer") => Self::DocumentViewer,
            n if n.ends_with("DocumentEditor") => Self::DocumentEditor,
            n if n.ends_with("ChangeStream") => Self::ChangeStream,
            n if n.starts_with("Redis") && n.ends_with("Console") => Self::RedisConsole,
            n if n.starts_with("Redis") => Self::Redis,
            _ => Self::Other,
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
    fn classifies_by_name() {
        assert_eq!(PanelKind::from_name("PgDataViewer"), PanelKind::DataViewer);
        assert_eq!(
            PanelKind::from_name("RedisConsole"),
            PanelKind::RedisConsole
        );
        assert_eq!(PanelKind::from_name("RedisKeys"), PanelKind::Redis);
        assert_eq!(
            PanelKind::from_name("SqliteQueryEditor"),
            PanelKind::QueryEditor
        );
        assert_eq!(PanelKind::from_name("Mystery"), PanelKind::Other);
    }
}
