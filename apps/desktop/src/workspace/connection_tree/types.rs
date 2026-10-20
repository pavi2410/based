use std::collections::HashSet;

use crate::workspace::TabSpec;
use gpui_kit::Hsla;
use gpui_kit::component::{IconName, Theme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Trigger,
    Collection,
    Key,
}

impl ObjectKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "matview",
            Self::Trigger => "trigger",
            Self::Collection => "collection",
            Self::Key => "key",
        }
    }

    pub fn group(&self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View | Self::MaterializedView => "Views",
            Self::Trigger => "Triggers",
            Self::Collection => "Collections",
            Self::Key => "Keys",
        }
    }

    /// Sidebar list icon (gpui-kit bundled SVG).
    pub(crate) fn list_icon(&self) -> IconName {
        match self {
            Self::Table => IconName::LayoutDashboard,
            Self::View => IconName::Eye,
            Self::MaterializedView => IconName::GalleryVerticalEnd,
            Self::Trigger => IconName::TriangleAlert,
            Self::Collection => IconName::Inbox,
            Self::Key => IconName::Asterisk,
        }
    }

    /// Kind color for catalog icons (table / view / collection / trigger).
    pub(crate) fn accent_color(&self, theme: &Theme) -> Hsla {
        match self {
            Self::Table => theme.blue_light,
            Self::View | Self::MaterializedView => theme.magenta_light,
            Self::Trigger => theme.warning_foreground,
            Self::Collection => theme.green_light,
            Self::Key => theme.red_light,
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

#[derive(Clone, Debug, Default)]
pub(crate) enum ConnCache {
    #[default]
    Idle,
    Loading,
    Ready(Vec<SchemaObject>),
    Error(String),
}

impl ConnCache {
    pub(crate) fn should_skip_load(&self) -> bool {
        matches!(self, Self::Loading | Self::Ready(_))
    }

    pub(crate) fn objects(&self) -> Option<&[SchemaObject]> {
        match self {
            Self::Ready(objects) => Some(objects),
            _ => None,
        }
    }

    pub(crate) fn start_loading(&mut self) {
        *self = Self::Loading;
    }

    pub(crate) fn set_ready(&mut self, objects: Vec<SchemaObject>) {
        *self = Self::Ready(objects);
    }

    pub(crate) fn set_error(&mut self, message: String) {
        *self = Self::Error(message);
    }

    pub(crate) fn invalidate(&mut self) {
        *self = Self::Idle;
    }
}

/// Per-connection explorer state: expanded Postgres schemas and cached objects.
#[derive(Default)]
pub(crate) struct ConnState {
    pub(crate) expanded_schemas: HashSet<String>,
    pub(crate) cache: ConnCache,
}

impl ConnState {
    pub(crate) fn cache(&self) -> &ConnCache {
        &self.cache
    }

    pub(crate) fn cache_mut(&mut self) -> &mut ConnCache {
        &mut self.cache
    }

    /// Expand `public` by default when no schema has been opened yet.
    pub(crate) fn seed_public_schema(&mut self, objects: &[SchemaObject]) {
        if self.expanded_schemas.is_empty()
            && objects
                .iter()
                .any(|o| o.schema.as_deref() == Some("public"))
        {
            self.expanded_schemas.insert("public".into());
        }
    }
}

#[derive(Clone)]
pub enum TreeEvent {
    OpenTab(TabSpec),
}
