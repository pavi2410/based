//! Empty state view — centered icon + title + body for panels with no data.

use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, v_flex};

use crate::app::prefs;

pub fn empty_state(
    title: &'static str,
    body: &'static str,
    icon: IconName,
    cx: &mut App,
) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(42.0))
                .h(px(42.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(cx.theme().border.opacity(0.82))
                .bg(cx.theme().muted.opacity(0.4))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .text_color(cx.theme().muted_foreground)
                        .with_size(prefs::ui_component_size(cx)),
                ),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(3.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                ),
        )
}
