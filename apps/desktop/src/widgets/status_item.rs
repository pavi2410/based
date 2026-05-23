//! Flat status bar segments — IDE-style label/value pairs without chip chrome.

use gpui::{Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};
use gpui_component::h_flex;

/// Muted vertical rule between status segments.
pub fn status_divider(muted: Hsla) -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(12.0))
        .flex_shrink_0()
        .bg(muted.opacity(0.22))
}

/// Plain label + value for the status rail (no border or background).
pub fn status_segment(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    muted: Hsla,
    fg: Hsla,
    dot: Option<Hsla>,
) -> impl IntoElement {
    h_flex()
        .h(px(22.0))
        .items_center()
        .gap(px(4.0))
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
        .child(div().text_xs().text_color(muted).child(label.into()))
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(fg.opacity(0.92))
                .child(value.into()),
        )
}

/// Single muted string for the right side of the status bar (e.g. version).
pub fn status_text(value: impl Into<SharedString>, muted: Hsla) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(muted.opacity(0.88))
        .child(value.into())
}
