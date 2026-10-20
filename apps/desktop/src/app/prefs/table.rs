//! Table preferences: grid behaviour, page size, query timeout.

use super::*;

/// Default SQL data viewer page size (rows per fetch).
pub const DEFAULT_PAGE_SIZE: u64 = 500;

/// Default query timeout shown in settings (seconds); execution wiring is future work.
pub const DEFAULT_QUERY_TIMEOUT_SECS: u32 = 30;

const PAGE_SIZE_MIN: u64 = 50;

const PAGE_SIZE_MAX: u64 = 5000;

const QUERY_TIMEOUT_MIN: u32 = 5;

const QUERY_TIMEOUT_MAX: u32 = 600;

pub(super) fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

pub(super) fn default_query_timeout_secs() -> u32 {
    DEFAULT_QUERY_TIMEOUT_SECS
}

/// Interaction and chrome toggles for data grids (gpui-kit DataTable).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablePreferences {
    #[serde(default = "default_true")]
    pub stripe: bool,
    #[serde(default = "default_true")]
    pub sortable: bool,
    #[serde(default = "default_true")]
    pub col_resizable: bool,
    #[serde(default = "default_true")]
    pub col_movable: bool,
    #[serde(default = "default_true")]
    pub row_selectable: bool,
    #[serde(default = "default_true")]
    pub cell_selectable: bool,
    #[serde(default = "default_true")]
    pub loop_selection: bool,
    #[serde(default = "default_true")]
    pub highlight_same_value: bool,
}

impl Default for TablePreferences {
    fn default() -> Self {
        Self {
            stripe: true,
            sortable: true,
            col_resizable: true,
            col_movable: true,
            row_selectable: true,
            cell_selectable: true,
            loop_selection: true,
            highlight_same_value: true,
        }
    }
}

pub fn table_size_token(cx: &App) -> SizeToken {
    cx.global::<NativePreferences>().chrome.table.size
}

pub fn page_size(cx: &App) -> u64 {
    cx.global::<NativePreferences>().page_size
}

pub fn query_timeout_secs(cx: &App) -> u32 {
    cx.global::<NativePreferences>().query_timeout_secs
}

pub fn table_prefs(cx: &App) -> TablePreferences {
    cx.global::<NativePreferences>().table_prefs
}

pub fn table_cell_size(cx: &App) -> Size {
    table_size_token(cx).to_component_size()
}

fn refresh_table_windows(changed: bool, cx: &mut App) {
    if changed {
        cx.refresh_windows();
    }
}

fn update_table_prefs(update: impl FnOnce(&mut TablePreferences) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.table_prefs);
        if changed {
            p.save_best_effort();
        }
        changed
    });
    refresh_table_windows(changed, cx);
}

pref_setter!(set_table_stripe, stripe: bool, update_table_prefs);

pref_setter!(set_table_sortable, sortable: bool, update_table_prefs);

pref_setter!(set_table_col_resizable, col_resizable: bool, update_table_prefs);

pref_setter!(set_table_col_movable, col_movable: bool, update_table_prefs);

pref_setter!(set_table_row_selectable, row_selectable: bool, update_table_prefs);

pref_setter!(set_table_cell_selectable, cell_selectable: bool, update_table_prefs);

pref_setter!(set_table_loop_selection, loop_selection: bool, update_table_prefs);

pref_setter!(set_table_highlight_same_value, highlight_same_value: bool, update_table_prefs);

pub fn set_table_size(size: SizeToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.table.size == size {
                return false;
            }
            c.table.size = size;
            true
        },
        cx,
    );
    cx.refresh_windows();
}

fn clamp_page_size(size: u64) -> u64 {
    size.clamp(PAGE_SIZE_MIN, PAGE_SIZE_MAX)
}

fn clamp_query_timeout(secs: u32) -> u32 {
    secs.clamp(QUERY_TIMEOUT_MIN, QUERY_TIMEOUT_MAX)
}

pub fn set_page_size(size: u64, cx: &mut App) {
    let size = clamp_page_size(size);
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.page_size == size {
            return;
        }
        p.page_size = size;
        p.save_best_effort();
    });
}

pub fn set_query_timeout_secs(secs: u32, cx: &mut App) {
    let secs = clamp_query_timeout(secs);
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.query_timeout_secs == secs {
            return;
        }
        p.query_timeout_secs = secs;
        p.save_best_effort();
    });
}
