//! Section eyebrow — muted bold xs label used as a list section header.

use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt, h_flex};

use crate::app::prefs;
use crate::widgets::layout::SIDEBAR_INSET;

/// Plain section header: bold xs muted label in a fixed-height row.
///
/// Standard layout: `h_8`, horizontal padding `px_3`. Suitable for sidebar
/// sections that don't need tree-indent alignment.
pub fn section_eyebrow(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    div()
        .h_8()
        .px_3()
        .flex()
        .items_center()
        .text_xs()
        .font_bold()
        .font_family(prefs::ui_font_family(cx))
        .font_weight(prefs::ui_font_weight(cx))
        .text_color(muted.opacity(0.86))
        .child(label.into())
}

/// Section header with an item count badge aligned to the right.
///
/// Standard layout: `px(SIDEBAR_INSET)`, `py_1`. Suitable for object-list
/// sections that need to show how many items are in the section.
pub fn section_eyebrow_counted(
    label: impl Into<SharedString>,
    count: usize,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    h_flex()
        .px(px(SIDEBAR_INSET))
        .py_1()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(muted.opacity(0.86))
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_family(prefs::code_font_family(cx))
                .text_color(muted.opacity(0.76))
                .child(count.to_string()),
        )
}
