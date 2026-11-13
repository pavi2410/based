use std::collections::HashSet;

use crate::workspace::TabSpec;
use gpui_kit::Hsla;
use gpui_kit::component::{IconName, Theme};

pub use based_core::{DbObject, ObjectKind};

/// How the sidebar draws an object kind.
pub(crate) trait ObjectKindUi {
    /// Sidebar list icon (gpui-kit bundled SVG).
    fn list_icon(self) -> IconName;
    /// Kind color for catalog icons (table / view / collection / trigger).
    fn accent_color(self, theme: &Theme) -> Hsla;
}

impl ObjectKindUi for ObjectKind {
    fn list_icon(self) -> IconName {
        match self {
            Self::Table => IconName::LayoutDashboard,
            Self::View => IconName::Eye,
            Self::MaterializedView => IconName::GalleryVerticalEnd,
            Self::Trigger => IconName::TriangleAlert,
            Self::Collection => IconName::Inbox,
            Self::Key => IconName::Asterisk,
        }
    }

    fn accent_color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Table => theme.blue_light,
            Self::View | Self::MaterializedView => theme.magenta_light,
            Self::Trigger => theme.warning_foreground,
            Self::Collection => theme.green_light,
            Self::Key => theme.red_light,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum ConnCache {
    #[default]
    Idle,
    Loading,
    Ready(Vec<DbObject>),
    Error(String),
}

impl ConnCache {
    /// Whether a load is in flight or settled. A failed load stays failed until
    /// the user retries (Retry / refresh); otherwise every frame would reload.
    pub(crate) fn should_skip_load(&self) -> bool {
        matches!(self, Self::Loading | Self::Ready(_) | Self::Error(_))
    }

    pub(crate) fn objects(&self) -> Option<&[DbObject]> {
        match self {
            Self::Ready(objects) => Some(objects),
            _ => None,
        }
    }

    pub(crate) fn start_loading(&mut self) {
        *self = Self::Loading;
    }

    pub(crate) fn set_ready(&mut self, objects: Vec<DbObject>) {
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
    /// Databases on the server (Postgres / Redis only).
    pub(crate) databases: Vec<String>,
}

impl ConnState {
    pub(crate) fn cache(&self) -> &ConnCache {
        &self.cache
    }

    pub(crate) fn cache_mut(&mut self) -> &mut ConnCache {
        &mut self.cache
    }

    /// Expand `public` by default when no schema has been opened yet.
    pub(crate) fn seed_public_schema(&mut self, objects: &[DbObject]) {
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

#[cfg(test)]
mod cache_tests {
    use super::ConnCache;

    #[test]
    fn failed_load_is_not_retried_until_invalidated() {
        let mut cache = ConnCache::default();
        assert!(!cache.should_skip_load());
        cache.start_loading();
        cache.set_error("permission denied for schema pg_catalog".into());
        assert!(
            cache.should_skip_load(),
            "a render must not restart a failed load"
        );
        cache.invalidate();
        assert!(
            !cache.should_skip_load(),
            "Retry invalidates, so the next render reloads"
        );
    }
}
