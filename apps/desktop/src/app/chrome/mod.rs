//! Visual chrome prefs: UI density scale and typography (fonts separate from theme colors).

mod density;
mod typography;

pub use density::{
    DensityPreset, SizeToken, TableProfile, panel_header_h, sidebar_row_gap, sidebar_row_py,
};
pub use typography::{
    CodeFontFamilyId, EditorFontProfile, FontWeightToken, UiFontFamilyId, UiFontProfile,
    apply_fonts,
};

use gpui::App;
use serde::{Deserialize, Serialize};

/// Persisted UI chrome: density preset + UI / editor / table profiles.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChromePrefs {
    #[serde(default)]
    pub density_preset: DensityPreset,
    #[serde(default)]
    pub ui: UiFontProfile,
    #[serde(default)]
    pub editor: EditorFontProfile,
    #[serde(default)]
    pub table: TableProfile,
}

impl Default for ChromePrefs {
    fn default() -> Self {
        let (ui_size, editor_size, table_size) = DensityPreset::Default.tokens();
        Self {
            density_preset: DensityPreset::Default,
            ui: UiFontProfile {
                size: ui_size,
                ..Default::default()
            },
            editor: EditorFontProfile {
                size: editor_size,
                ..Default::default()
            },
            table: TableProfile { size: table_size },
        }
    }
}

impl ChromePrefs {
    pub fn apply_density_preset(&mut self, preset: DensityPreset) {
        if preset == DensityPreset::Custom {
            return;
        }
        let (ui_size, editor_size, table_size) = preset.tokens();
        self.density_preset = preset;
        self.ui.size = ui_size;
        self.editor.size = editor_size;
        self.table.size = table_size;
    }

    pub fn sync_density_preset(&mut self) {
        for preset in DensityPreset::SELECTABLE {
            let (ui, editor, table) = preset.tokens();
            if self.ui.size == ui && self.editor.size == editor && self.table.size == table {
                self.density_preset = preset;
                return;
            }
        }
        self.density_preset = DensityPreset::Custom;
    }

    pub fn migrate_from_legacy(
        &mut self,
        ui_font_size: f32,
        mono_font_size: f32,
        table_compact: bool,
    ) {
        self.ui.size = SizeToken::from_legacy_px(ui_font_size);
        self.editor.size = SizeToken::from_legacy_px(mono_font_size);
        self.table.size = if table_compact {
            SizeToken::XSmall
        } else {
            SizeToken::Small
        };
        self.sync_density_preset();
    }
}

/// Apply chrome font profiles to the global theme and refresh windows.
pub fn apply_chrome(prefs: &ChromePrefs, cx: &mut App) {
    apply_fonts(&prefs.ui, &prefs.editor, cx);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_round_trip() {
        let mut prefs = ChromePrefs::default();
        prefs.apply_density_preset(DensityPreset::Compact);
        assert_eq!(prefs.ui.size, SizeToken::XSmall);
        assert_eq!(prefs.editor.size, SizeToken::Small);
        prefs.sync_density_preset();
        assert_eq!(prefs.density_preset, DensityPreset::Compact);
    }
}
