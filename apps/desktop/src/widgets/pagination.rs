//! SQL offset/limit paging helpers and gpui-kit `Pagination` builder.

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    pagination::Pagination,
};
use gpui_kit::{AnyElement, App, ElementId, IntoElement, ParentElement, Styled, div, px};

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

/// `12400` as `12,400`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Human-readable row range, e.g. `1–50 of 1,000`.
pub fn sql_row_range_label(total: u64, offset: u64, page_size: u64) -> String {
    if total == 0 {
        return "0 rows".to_string();
    }
    let start = offset + 1;
    let end = (offset + page_size).min(total);
    format!(
        "{}–{} of {}",
        thousands(start),
        thousands(end),
        thousands(total)
    )
}

/// Page numbers to show: first, last and the neighbours of `current`; `None` is a gap.
pub fn page_window(current: usize, total: usize) -> Vec<Option<usize>> {
    let mut out = Vec::new();
    let mut last = 0;
    for page in 1..=total {
        if page == 1 || page == total || page.abs_diff(current) <= 1 {
            if page > last + 1 {
                out.push(None);
            }
            out.push(Some(page));
            last = page;
        }
    }
    out
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

/// Row range and a numbered pager in one compact group (for the footer).
pub fn sql_pagination_group(
    id: impl Into<ElementId>,
    total: u64,
    offset: u64,
    page_size: u64,
    disabled: bool,
    on_page: impl Fn(usize, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let id: ElementId = id.into();
    let (page, pages) = sql_page_state(total, offset, page_size);
    let muted = cx.theme().muted_foreground;
    let on_page = Rc::new(on_page);
    let step = |name: &'static str, icon: IconName, target: usize, off: bool| {
        let on_page = on_page.clone();
        Button::new((id.clone(), name))
            .ghost()
            .xsmall()
            .icon(icon)
            .disabled(disabled || off)
            .on_click(move |_, _, cx| on_page(target, cx))
    };

    h_flex()
        .flex_shrink_0()
        .gap(px(10.0))
        .items_center()
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(sql_row_range_label(total, offset, page_size)),
        )
        .child(
            h_flex()
                .gap(px(2.0))
                .items_center()
                .child(step(
                    "prev",
                    IconName::ChevronLeft,
                    page.saturating_sub(1),
                    page <= 1,
                ))
                .children(page_window(page, pages).into_iter().map(|entry| {
                    match entry {
                        None => div()
                            .px(px(4.0))
                            .text_xs()
                            .text_color(muted)
                            .child("…")
                            .into_any_element(),
                        Some(n) => {
                            let on_page = on_page.clone();
                            let button = Button::new((id.clone(), format!("p{n}")))
                                .xsmall()
                                .label(thousands(n as u64))
                                .disabled(disabled)
                                .on_click(move |_, _, cx| on_page(n, cx));
                            if n == page {
                                button.outline().into_any_element()
                            } else {
                                button.ghost().into_any_element()
                            }
                        }
                    }
                }))
                .child(step(
                    "next",
                    IconName::ChevronRight,
                    page + 1,
                    page >= pages,
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{page_window, thousands};

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(12400), "12,400");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn window_elides_distant_pages() {
        let show = |c, t| page_window(c, t);
        assert_eq!(show(1, 3), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(
            show(5, 124),
            vec![Some(1), None, Some(4), Some(5), Some(6), None, Some(124)]
        );
        assert_eq!(show(1, 124), vec![Some(1), Some(2), None, Some(124)]);
    }
}
