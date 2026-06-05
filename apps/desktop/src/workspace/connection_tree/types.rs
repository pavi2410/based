use std::collections::HashSet;

use crate::connection::EngineKind;
use crate::workspace::TabSpec;
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

#[derive(Clone, Debug, Default)]
pub(crate) enum ConnCache {
    #[default]
    Idle,
    Loading,
    Ready(Vec<SchemaObject>),
    Error(String),
}

impl ConnCache {
    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

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
}

/// Per-connection explorer expansion and cached schema objects.
pub(crate) enum ConnState {
    Postgres {
        expanded: bool,
        expanded_schemas: HashSet<String>,
        cache: ConnCache,
    },
    Sqlite {
        expanded: bool,
        cache: ConnCache,
    },
    MongoDB {
        expanded: bool,
        cache: ConnCache,
    },
}

impl ConnState {
    pub(crate) fn new(engine: EngineKind) -> Self {
        match engine {
            EngineKind::Postgres => Self::Postgres {
                expanded: false,
                expanded_schemas: HashSet::new(),
                cache: ConnCache::default(),
            },
            EngineKind::SQLite => Self::Sqlite {
                expanded: false,
                cache: ConnCache::default(),
            },
            EngineKind::MongoDB => Self::MongoDB {
                expanded: false,
                cache: ConnCache::default(),
            },
        }
    }

    pub(crate) fn expanded(&self) -> bool {
        match self {
            Self::Postgres { expanded, .. }
            | Self::Sqlite { expanded, .. }
            | Self::MongoDB { expanded, .. } => *expanded,
        }
    }

    pub(crate) fn set_expanded(&mut self, value: bool) {
        match self {
            Self::Postgres { expanded, .. }
            | Self::Sqlite { expanded, .. }
            | Self::MongoDB { expanded, .. } => *expanded = value,
        }
    }

    pub(crate) fn cache(&self) -> &ConnCache {
        match self {
            Self::Postgres { cache, .. }
            | Self::Sqlite { cache, .. }
            | Self::MongoDB { cache, .. } => cache,
        }
    }

    pub(crate) fn cache_mut(&mut self) -> &mut ConnCache {
        match self {
            Self::Postgres { cache, .. }
            | Self::Sqlite { cache, .. }
            | Self::MongoDB { cache, .. } => cache,
        }
    }

    pub(crate) fn postgres_schemas(&mut self) -> Option<&mut HashSet<String>> {
        match self {
            Self::Postgres {
                expanded_schemas, ..
            } => Some(expanded_schemas),
            _ => None,
        }
    }

    pub(crate) fn seed_postgres_public_schema(&mut self, objects: &[SchemaObject]) {
        let Some(schemas) = self.postgres_schemas() else {
            return;
        };
        if !schemas.is_empty() {
            return;
        }
        if objects
            .iter()
            .any(|o| o.schema.as_deref() == Some("public"))
        {
            schemas.insert("public".into());
        }
    }
}

#[derive(Clone)]
pub enum TreeEvent {
    OpenTab(TabSpec),
}
