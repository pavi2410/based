//! SQL offset/limit paging helpers and gpui-component `Pagination` builder.

use gpui::{ElementId, Styled, px};
use gpui_component::{Disableable, Sizable, pagination::Pagination};

/// 1-based current page and total page count for offset/limit SQL paging.
pub fn sql_page_state(total: u64, offset: u64, page_size: u64) -> (usize, usize) {
    let page_size = page_size.max(1);
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(page_size) as usize
    };
    let current_page = (offset / page_size + 1) as usize;
    (current_page.min(total_pages).max(1), total_pages.max(1))
}

/// Zero-based row offset for a 1-based page index.
pub fn offset_for_page(page_1_based: usize, page_size: u64) -> u64 {
    page_1_based.saturating_sub(1) as u64 * page_size.max(1)
}

/// Human-readable row range for the toolbar pill.
pub fn sql_row_range_label(total: u64, offset: u64, page_size: u64) -> String {
    if total == 0 {
        return "0 rows".to_string();
    }
    let start = offset + 1;
    let end = (offset + page_size).min(total);
    format!("{start} – {end} of {total}")
}

/// Compact prev/next pager; attach `.on_click` with a 1-based page index handler.
pub fn sql_pagination_controls(
    id: impl Into<ElementId>,
    total: u64,
    offset: u64,
    page_size: u64,
    disabled: bool,
) -> Pagination {
    let (current_page, total_pages) = sql_page_state(total, offset, page_size);
    Pagination::new(id)
        .compact()
        .small()
        .py(px(0.0))
        .px(px(0.0))
        .current_page(current_page)
        .total_pages(total_pages)
        .disabled(disabled)
}
