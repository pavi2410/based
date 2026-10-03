//! Shared appearance + theme controls for settings.

use gpui_kit::component::{
    ThemeMode, ThemeRegistry,
    searchable_list::SearchableListItem,
    select::{Select, SelectState},
};
use gpui_kit::{App, Entity, IntoElement, SharedString, Window};

use crate::app::prefs;

/// Which settings dropdown axis a preview session tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreviewAxis {
    Light,
    Dark,
}

/// Tracks a non-persisted theme preview while browsing a settings dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePreviewSession {
    pub axis: ThemePreviewAxis,
    active: bool,
    last_name: Option<String>,
}

impl ThemePreviewSession {
    pub fn new(axis: ThemePreviewAxis) -> Self {
        Self {
            axis,
            active: false,
            last_name: None,
        }
    }

    pub fn preview(&mut self, name: &str, window: Option<&mut Window>, cx: &mut App) {
        let committed = match self.axis {
            ThemePreviewAxis::Light => prefs::light_theme_name(cx),
            ThemePreviewAxis::Dark => prefs::dark_theme_name(cx),
        };
        if name == committed {
            if self.active {
                self.revert(window, cx);
            }
            return;
        }
        if self.last_name.as_deref() == Some(name) {
            return;
        }
        match self.axis {
            ThemePreviewAxis::Light => prefs::preview_light_theme(name, window, cx),
            ThemePreviewAxis::Dark => prefs::preview_dark_theme(name, window, cx),
        }
        self.active = true;
        self.last_name = Some(name.to_string());
    }

    pub fn revert(&mut self, window: Option<&mut Window>, cx: &mut App) {
        if !self.active {
            return;
        }
        match self.axis {
            ThemePreviewAxis::Light => prefs::revert_light_theme_preview(window, cx),
            ThemePreviewAxis::Dark => prefs::revert_dark_theme_preview(window, cx),
        }
        self.active = false;
        self.last_name = None;
    }

    pub fn clear_after_commit(&mut self) {
        self.active = false;
        self.last_name = None;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Registry theme name row for settings dropdowns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeNameItem {
    name: SharedString,
}

impl ThemeNameItem {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }

    pub fn items_for_mode(mode: ThemeMode, cx: &App) -> Vec<Self> {
        ThemeRegistry::global(cx)
            .sorted_themes()
            .into_iter()
            .filter(|theme| theme.mode == mode)
            .filter(|theme| !is_hidden_registry_theme(theme.name.as_ref()))
            .map(|theme| Self::new(theme.name.clone()))
            .collect()
    }
}

fn is_hidden_registry_theme(name: &str) -> bool {
    matches!(name, "Default Light" | "Default Dark")
}

impl SearchableListItem for ThemeNameItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

const BUNDLE_THEMES: &[&str] = &[
    include_str!("bundles/based.json"),
    include_str!("bundles/gruvbox.json"),
    include_str!("bundles/ayu.json"),
    include_str!("bundles/catppuccin.json"),
    include_str!("bundles/everforest.json"),
    include_str!("bundles/solarized.json"),
    include_str!("bundles/github_high_contrast.json"),
    include_str!("bundles/github_colorblind.json"),
];

/// Load bundled third-party theme JSON into the registry (idempotent).
pub fn load_bundled_themes(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for content in BUNDLE_THEMES {
        if let Err(err) = registry.load_themes_from_str(content) {
            log::warn!("theme bundle load: {err:#}");
        }
    }
}

/// Theme name picker for settings (dropdown).
pub fn theme_name_select(select: Entity<SelectState<Vec<ThemeNameItem>>>) -> impl IntoElement {
    Select::new(&select).w_full()
}
