//! Sizing constants and computed layout metrics shared across the widget layer.

use gpui::App;

use crate::app::prefs;
use crate::app::prefs::{panel_header_h, sidebar_row_gap, sidebar_row_py};

/// Boxy panel corner radius (Linear / Vercel–style).
pub const PANEL_RADIUS: f32 = 4.0;
/// Horizontal inset for sidebar list rows and section headers.
pub const SIDEBAR_INSET: f32 = 8.0;
/// Fixed lead column so engine icons align across connected / disconnected rows.
pub const CONNECTION_CHEVRON_SLOT_W: f32 = 18.0;
/// Schema object row kind icon size.
pub const SCHEMA_ROW_ICON_SIZE: f32 = 14.0;
/// Horizontal step between browser-tree nesting levels (schema → kind → leaf).
pub const BROWSER_TREE_INDENT_STEP: f32 = 10.0;

pub fn panel_header_height(cx: &App) -> f32 {
    panel_header_h(prefs::ui_size_token(cx))
}

pub fn sidebar_row_padding_y(cx: &App) -> f32 {
    sidebar_row_py(prefs::ui_size_token(cx))
}

pub fn sidebar_row_inner_gap(cx: &App) -> f32 {
    sidebar_row_gap(prefs::ui_size_token(cx))
}

/// Browser tree: left edge of the engine-icon column on connection rows.
pub fn browser_tree_engine_col(cx: &App) -> f32 {
    SIDEBAR_INSET + CONNECTION_CHEVRON_SLOT_W + sidebar_row_inner_gap(cx)
}

pub struct BrowserTreeIndent {
    base: f32,
}

impl BrowserTreeIndent {
    pub fn from_app(cx: &App) -> Self {
        Self {
            base: browser_tree_engine_col(cx),
        }
    }

    /// 1-indexed depth under a connection row (1 = first child, 2 = second, …).
    pub fn pl(&self, level: u32) -> f32 {
        debug_assert!(level >= 1);
        self.base + BROWSER_TREE_INDENT_STEP * (level - 1) as f32
    }
}
