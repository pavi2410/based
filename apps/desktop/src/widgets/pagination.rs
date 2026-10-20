//! SQL offset/limit paging helpers and gpui-kit `Pagination` builder.

use gpui_kit::component::{ActiveTheme, Disableable, Sizable, h_flex, pagination::Pagination};
use gpui_kit::{App, ElementId, IntoElement, ParentElement, Styled, div, px};

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

/// Human-readable row range, e.g. `1 – 50 of 1000`.
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

/// Single bar with the row range on the left and the pager on the right.
pub fn sql_pagination_bar(
    id: impl Into<ElementId>,
    total: u64,
    offset: u64,
    page_size: u64,
    disabled: bool,
    on_page: impl Fn(usize, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let (page, pages) = sql_page_state(total, offset, page_size);
    h_flex()
        .h(px(32.0))
        .px(px(8.0))
        .gap(px(8.0))
        .items_center()
        .flex_shrink_0()
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(sql_row_range_label(total, offset, page_size)),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("Page {page} of {pages}")),
        )
        .child(
            sql_pagination_controls(id, total, offset, page_size, disabled)
                .on_click(move |page, _, cx| on_page(*page, cx)),
        )
}
