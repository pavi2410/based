//! Metadata pill chip — compact label/value badge for panel toolbars.

use crate::connection::Environment;
use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;

pub fn metadata_pill(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    h_flex()
        .h(px(20.0))
        .items_center()
        .gap(px(5.0))
        .px(px(7.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.34))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_family(prefs::code_font_family(cx))
                .text_color(cx.theme().foreground.opacity(0.9))
                .child(value.into()),
        )
}

/// Small coloured pill naming a connection's environment (production, staging, ...).
pub fn env_pill(env: Environment, cx: &App) -> impl IntoElement {
    let tone = env.color(cx.theme());
    div()
        .px(px(6.0))
        .rounded_full()
        .border_1()
        .border_color(tone.opacity(0.5))
        .bg(tone.opacity(0.12))
        .text_color(tone)
        .text_xs()
        .child(env.label())
}
