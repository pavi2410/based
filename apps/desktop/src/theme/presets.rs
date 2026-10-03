//! Curated light/dark theme pairs available in Based.

/// A selectable theme family (light + dark variant names in [`ThemeRegistry`](gpui_kit::component::ThemeRegistry)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePreset {
    pub id: &'static str,
    pub light_name: &'static str,
    pub dark_name: &'static str,
}

pub const DEFAULT_LIGHT_THEME: &str = "Based Light";
pub const DEFAULT_DARK_THEME: &str = "Based Dark";

pub const PRESETS: &[ThemePreset] = &[
    ThemePreset {
        id: "based",
        light_name: "Based Light",
        dark_name: "Based Dark",
    },
    ThemePreset {
        id: "gruvbox",
        light_name: "Gruvbox Light",
        dark_name: "Gruvbox Dark",
    },
    ThemePreset {
        id: "catppuccin",
        light_name: "Catppuccin Latte",
        dark_name: "Catppuccin Mocha",
    },
    ThemePreset {
        id: "ayu",
        light_name: "Ayu Light",
        dark_name: "Ayu Dark",
    },
    ThemePreset {
        id: "everforest",
        light_name: "Everforest Light",
        dark_name: "Everforest Dark",
    },
    ThemePreset {
        id: "solarized",
        light_name: "Solarized Light",
        dark_name: "Solarized Dark",
    },
];

pub fn preset_by_id(id: &str) -> Option<&'static ThemePreset> {
    PRESETS.iter().find(|p| p.id == id)
}
