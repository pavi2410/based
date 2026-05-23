//! Tab label: engine-colored prefix + title + optional dirty dot.

use gpui::{FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px};
use gpui_component::ActiveTheme;

use crate::connection::EngineKind;
use crate::widgets::ui::{engine_color, engine_label};

pub fn tab_chip(
    engine: EngineKind,
    title: impl Into<SharedString>,
    dirty: bool,
    disconnected: bool,
    cx: &gpui::App,
) -> impl IntoElement {
    let color = if disconnected {
        cx.theme().muted_foreground.opacity(0.55)
    } else {
        engine_color(engine)
    };
    let title = title.into();

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .max_w(px(220.0))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(color)
                .child(format!("({})", engine_label(engine))),
        )
        .child(
            div()
                .text_sm()
                .truncate()
                .text_color(if disconnected {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().foreground
                })
                .child(title),
        )
        .when(dirty, |row| {
            row.child(div().text_xs().text_color(cx.theme().accent).child("●"))
        })
}
