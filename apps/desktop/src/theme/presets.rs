//! Curated light/dark theme pairs available in Based.

/// A selectable theme family (light + dark variant names in [`ThemeRegistry`](gpui_component::ThemeRegistry)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePreset {
    pub id: &'static str,
    pub label: &'static str,
    pub light_name: &'static str,
    pub dark_name: &'static str,
    /// Swatch color for preset card previews (light side).
    pub preview_light: &'static str,
    /// Swatch color for preset card previews (dark side).
    pub preview_dark: &'static str,
}

pub const DEFAULT_PRESET_ID: &str = "based";

pub const DEFAULT_LIGHT_THEME: &str = "Based Light";
pub const DEFAULT_DARK_THEME: &str = "Based Dark";

/// Onboarding shows three paired families (Zed-style).
pub const ONBOARDING_PRESET_IDS: &[&str] = &["based", "ayu", "gruvbox"];

pub const PRESETS: &[ThemePreset] = &[
    ThemePreset {
        id: "based",
        label: "Based",
        light_name: "Based Light",
        dark_name: "Based Dark",
        preview_light: "#ffffff",
        preview_dark: "#0a0a0a",
    },
    ThemePreset {
        id: "gruvbox",
        label: "Gruvbox",
        light_name: "Gruvbox Light",
        dark_name: "Gruvbox Dark",
        preview_light: "#fbf1c7",
        preview_dark: "#282828",
    },
    ThemePreset {
        id: "catppuccin",
        label: "Catppuccin",
        light_name: "Catppuccin Latte",
        dark_name: "Catppuccin Mocha",
        preview_light: "#eff1f5",
        preview_dark: "#1e1e2e",
    },
    ThemePreset {
        id: "ayu",
        label: "Ayu",
        light_name: "Ayu Light",
        dark_name: "Ayu Dark",
        preview_light: "#fcfcfc",
        preview_dark: "#0f1419",
    },
    ThemePreset {
        id: "everforest",
        label: "Everforest",
        light_name: "Everforest Light",
        dark_name: "Everforest Dark",
        preview_light: "#efefef",
        preview_dark: "#2d353b",
    },
    ThemePreset {
        id: "solarized",
        label: "Solarized",
        light_name: "Solarized Light",
        dark_name: "Solarized Dark",
        preview_light: "#fdf6e3",
        preview_dark: "#002b36",
    },
];

pub fn preset_by_id(id: &str) -> Option<&'static ThemePreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn onboarding_presets() -> impl Iterator<Item = &'static ThemePreset> {
    ONBOARDING_PRESET_IDS
        .iter()
        .filter_map(|id| preset_by_id(id))
}

pub fn preset_id_for_pair(light_name: &str, dark_name: &str) -> Option<&'static str> {
    PRESETS
        .iter()
        .find(|p| p.light_name == light_name && p.dark_name == dark_name)
        .map(|p| p.id)
}
