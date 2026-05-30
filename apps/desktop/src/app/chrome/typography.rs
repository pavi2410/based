//! Font families, weights, and theme application for UI and code editors.

use gpui::{App, FontWeight, SharedString, px};
use gpui_component::Theme;
use serde::{Deserialize, Serialize};

use super::density::SizeToken;

/// Font weight token (maps to GPUI weights, not raw CSS units in UI).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontWeightToken {
    Light,
    #[default]
    Regular,
    Medium,
    Semibold,
}

impl FontWeightToken {
    pub const ALL: [Self; 4] = [Self::Light, Self::Regular, Self::Medium, Self::Semibold];

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Regular => "Regular",
            Self::Medium => "Medium",
            Self::Semibold => "Semibold",
        }
    }

    pub fn to_gpui(self) -> FontWeight {
        match self {
            Self::Light => FontWeight::LIGHT,
            Self::Regular => FontWeight::NORMAL,
            Self::Medium => FontWeight::MEDIUM,
            Self::Semibold => FontWeight::SEMIBOLD,
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Regular => "regular",
            Self::Medium => "medium",
            Self::Semibold => "semibold",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "light" => Some(Self::Light),
            "regular" => Some(Self::Regular),
            "medium" => Some(Self::Medium),
            "semibold" => Some(Self::Semibold),
            _ => None,
        }
    }
}

/// UI font family (interface chrome).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiFontFamilyId {
    #[default]
    IbmPlexMono,
    SystemUi,
    JetBrainsMono,
    PlatformMono,
}

impl UiFontFamilyId {
    pub const ALL: [Self; 4] = [
        Self::IbmPlexMono,
        Self::SystemUi,
        Self::JetBrainsMono,
        Self::PlatformMono,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::IbmPlexMono => "IBM Plex Mono",
            Self::SystemUi => "System UI",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::PlatformMono => "System Mono",
        }
    }

    pub fn resolve(self) -> SharedString {
        match self {
            Self::IbmPlexMono => "IBM Plex Mono".into(),
            Self::SystemUi => ".SystemUIFont".into(),
            Self::JetBrainsMono => "JetBrains Mono".into(),
            Self::PlatformMono => platform_mono_family(),
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::IbmPlexMono => "ibm_plex_mono",
            Self::SystemUi => "system_ui",
            Self::JetBrainsMono => "jetbrains_mono",
            Self::PlatformMono => "platform_mono",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "ibm_plex_mono" => Some(Self::IbmPlexMono),
            "system_ui" => Some(Self::SystemUi),
            "jetbrains_mono" => Some(Self::JetBrainsMono),
            "platform_mono" => Some(Self::PlatformMono),
            _ => None,
        }
    }
}

/// Code / editor font family.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeFontFamilyId {
    #[default]
    JetBrainsMono,
    IbmPlexMono,
    PlatformMono,
}

impl CodeFontFamilyId {
    pub const ALL: [Self; 3] = [Self::JetBrainsMono, Self::IbmPlexMono, Self::PlatformMono];

    pub fn label(self) -> &'static str {
        match self {
            Self::JetBrainsMono => "JetBrains Mono",
            Self::IbmPlexMono => "IBM Plex Mono",
            Self::PlatformMono => "System Mono",
        }
    }

    pub fn resolve(self) -> SharedString {
        match self {
            Self::JetBrainsMono => "JetBrains Mono".into(),
            Self::IbmPlexMono => "IBM Plex Mono".into(),
            Self::PlatformMono => platform_mono_family(),
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::JetBrainsMono => "jetbrains_mono",
            Self::IbmPlexMono => "ibm_plex_mono",
            Self::PlatformMono => "platform_mono",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "jetbrains_mono" => Some(Self::JetBrainsMono),
            "ibm_plex_mono" => Some(Self::IbmPlexMono),
            "platform_mono" => Some(Self::PlatformMono),
            _ => None,
        }
    }
}

fn platform_mono_family() -> SharedString {
    if cfg!(target_os = "macos") {
        "Menlo".into()
    } else if cfg!(target_os = "windows") {
        "Consolas".into()
    } else {
        "DejaVu Sans Mono".into()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiFontProfile {
    pub family: UiFontFamilyId,
    pub weight: FontWeightToken,
    pub size: SizeToken,
}

impl Default for UiFontProfile {
    fn default() -> Self {
        Self {
            family: UiFontFamilyId::IbmPlexMono,
            weight: FontWeightToken::Regular,
            size: SizeToken::Medium,
        }
    }
}

/// Code / editor typography profile.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorFontProfile {
    pub family: CodeFontFamilyId,
    pub weight: FontWeightToken,
    pub size: SizeToken,
}

impl Default for EditorFontProfile {
    fn default() -> Self {
        Self {
            family: CodeFontFamilyId::JetBrainsMono,
            weight: FontWeightToken::Regular,
            size: SizeToken::Medium,
        }
    }
}

/// Apply font profiles to the global theme.
pub fn apply_fonts(ui: &UiFontProfile, editor: &EditorFontProfile, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.font_family = ui.family.resolve();
    theme.font_size = px(ui.size.ui_px());
    theme.mono_font_family = editor.family.resolve();
    theme.mono_font_size = px(editor.size.editor_px());
}
