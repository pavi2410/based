use crate::connection::EngineKind;
use crate::workspace::tab_spec::TabSpec;
use gpui_component::IconName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Trigger,
    Collection,
}

impl ObjectKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "matview",
            Self::Trigger => "trigger",
            Self::Collection => "collection",
        }
    }

    /// Short badge for sidebar object rows (fits fixed-width column).
    pub(crate) fn badge_label(&self) -> &'static str {
        match self {
            Self::Table => "tbl",
            Self::View => "view",
            Self::MaterializedView => "mview",
            Self::Trigger => "trig",
            Self::Collection => "coll",
        }
    }

    pub fn group(&self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View | Self::MaterializedView => "Views",
            Self::Trigger => "Triggers",
            Self::Collection => "Collections",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Table => "▤",
            Self::View | Self::MaterializedView => "◈",
            Self::Trigger => "⚡",
            Self::Collection => "▦",
        }
    }

    /// Sidebar list icon (gpui-component bundled SVG).
    pub(crate) fn list_icon(&self) -> IconName {
        match self {
            Self::Table => IconName::LayoutDashboard,
            Self::View => IconName::Eye,
            Self::MaterializedView => IconName::GalleryVerticalEnd,
            Self::Trigger => IconName::TriangleAlert,
            Self::Collection => IconName::Inbox,
        }
    }
}

/// Schema browser row (PostgreSQL exposes `schema` + local name).
#[derive(Clone, Debug)]
pub struct SchemaObject {
    pub name: String,
    pub schema: Option<String>,
    pub kind: ObjectKind,
}

impl SchemaObject {
    pub fn display_name(&self) -> String {
        if let Some(schema) = &self.schema {
            format!("{schema}.{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ActiveObjects {
    Empty,
    Loading {
        label: String,
        engine: EngineKind,
    },
    Ready {
        label: String,
        engine: EngineKind,
        objects: Vec<SchemaObject>,
    },
    Error {
        label: String,
        message: String,
    },
}

/// Reserved for per-connection expansion / cached schema.
pub(crate) struct ConnState {
    pub expanded: bool,
    pub objects: Option<Vec<SchemaObject>>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum TreeEvent {
    OpenTab(TabSpec),
}
