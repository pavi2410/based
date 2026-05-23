//! Status bar chips — at-a-glance contextual facts.

use gpui::{Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};
use gpui_component::h_flex;

/// Small boxed label + value for the status rail.
pub fn status_chip(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    muted: Hsla,
    fg: Hsla,
    dot: Option<Hsla>,
) -> impl IntoElement {
    h_flex()
        .h(px(18.0))
        .items_center()
        .gap(px(5.0))
        .px(px(7.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(muted.opacity(0.35))
        .bg(muted.opacity(0.12))
        .when_some(dot, |row, color| {
            row.child(
                div()
                    .w(px(5.0))
                    .h(px(5.0))
                    .rounded_full()
                    .flex_shrink_0()
                    .bg(color),
            )
        })
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(fg.opacity(0.92))
                .child(value.into()),
        )
}
