//! UI density: size tokens, presets, and layout scale helpers.

use gpui_component::Size;
use serde::{Deserialize, Serialize};

/// UI / editor / table size token.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SizeToken {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl SizeToken {
    pub const ALL: [Self; 4] = [Self::XSmall, Self::Small, Self::Medium, Self::Large];

    pub fn label(self) -> &'static str {
        match self {
            Self::XSmall => "XSmall",
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    pub fn ui_px(self) -> f32 {
        match self {
            Self::XSmall => 12.0,
            Self::Small => 13.0,
            Self::Medium => 14.0,
            Self::Large => 16.0,
        }
    }

    pub fn editor_px(self) -> f32 {
        self.ui_px()
    }

    pub fn to_component_size(self) -> Size {
        match self {
            Self::XSmall => Size::XSmall,
            Self::Small => Size::Small,
            Self::Medium => Size::Medium,
            Self::Large => Size::Large,
        }
    }

    pub fn from_legacy_px(px: f32) -> Self {
        if px <= 11.0 {
            Self::XSmall
        } else if px <= 13.0 {
            Self::Small
        } else if px <= 15.0 {
            Self::Medium
        } else {
            Self::Large
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::XSmall => "xsmall",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "xsmall" => Some(Self::XSmall),
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }
}

/// Density preset bundles UI, editor, and table size tokens.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DensityPreset {
    Compact,
    #[default]
    Default,
    Comfortable,
    Custom,
}

impl DensityPreset {
    pub const SELECTABLE: [Self; 3] = [Self::Compact, Self::Default, Self::Comfortable];

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Default => "Default",
            Self::Comfortable => "Comfortable",
            Self::Custom => "Custom",
        }
    }

    pub fn tokens(self) -> (SizeToken, SizeToken, SizeToken) {
        match self {
            Self::Compact => (SizeToken::XSmall, SizeToken::Small, SizeToken::XSmall),
            Self::Default => (SizeToken::Medium, SizeToken::Medium, SizeToken::XSmall),
            Self::Comfortable => (SizeToken::Medium, SizeToken::Large, SizeToken::Small),
            Self::Custom => (SizeToken::Medium, SizeToken::Medium, SizeToken::XSmall),
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Default => "default",
            Self::Comfortable => "comfortable",
            Self::Custom => "custom",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "compact" => Some(Self::Compact),
            "default" => Some(Self::Default),
            "comfortable" => Some(Self::Comfortable),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableProfile {
    pub size: SizeToken,
}

impl Default for TableProfile {
    fn default() -> Self {
        Self {
            size: SizeToken::XSmall,
        }
    }
}

/// Layout density helpers (Medium = current hardcoded defaults).
pub fn sidebar_row_py(size: SizeToken) -> f32 {
    match size {
        SizeToken::XSmall => 1.0,
        SizeToken::Small => 2.0,
        SizeToken::Medium => 3.0,
        SizeToken::Large => 5.0,
    }
}

pub fn sidebar_row_gap(size: SizeToken) -> f32 {
    match size {
        SizeToken::XSmall => 2.0,
        SizeToken::Small => 2.0,
        SizeToken::Medium => 3.0,
        SizeToken::Large => 4.0,
    }
}

pub fn panel_header_h(size: SizeToken) -> f32 {
    match size {
        SizeToken::XSmall => 26.0,
        SizeToken::Small => 28.0,
        SizeToken::Medium => 32.0,
        SizeToken::Large => 36.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_px_migration() {
        assert_eq!(SizeToken::from_legacy_px(10.0), SizeToken::XSmall);
        assert_eq!(SizeToken::from_legacy_px(14.0), SizeToken::Medium);
        assert_eq!(SizeToken::from_legacy_px(18.0), SizeToken::Large);
    }
}
