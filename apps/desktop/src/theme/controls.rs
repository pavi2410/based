//! Theme registry loading and settings dropdown rows.

use gpui_kit::component::{ThemeMode, ThemeRegistry, searchable_list::SearchableListItem};
use gpui_kit::{App, SharedString};

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
