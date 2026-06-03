use gpui::{Entity, FontWeight, IntoElement, ParentElement, div, prelude::*, px};
use gpui_component::{ActiveTheme, h_flex, list::List, list::ListState, v_flex};

use crate::widgets::SIDEBAR_INSET;

use super::connection_list::ConnectionListDelegate;
use super::object_list::ObjectListDelegate;
use super::types::ActiveObjects;

pub(super) fn render_connections_pane(
    list: Entity<ListState<ConnectionListDelegate>>,
    cx: &mut gpui::App,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let muted = cx.theme().muted_foreground;

    v_flex()
        .flex_1()
        .min_h(gpui::px(120.0))
        .min_h_0()
        .border_b_1()
        .border_color(border)
        .child(
            h_flex()
                .h(gpui::px(32.0))
                .px(px(SIDEBAR_INSET))
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .truncate()
                        .child("Connections"),
                )
                .child(
                    div()
                        .h(gpui::px(22.0))
                        .px(gpui::px(7.0))
                        .rounded(gpui::px(5.0))
                        .border_1()
                        .border_color(border.opacity(0.8))
                        .bg(cx.theme().muted.opacity(0.38))
                        .text_xs()
                        .text_color(muted)
                        .flex()
                        .items_center()
                        .child("+"),
                ),
        )
        .child(
            List::new(&list)
                .flex_1()
                .min_h_0()
                .w_full()
                .search_placeholder("Search connections"),
        )
}

pub(super) fn render_objects_pane(
    active_objects: ActiveObjects,
    list: Entity<ListState<ObjectListDelegate>>,
    cx: &mut gpui::App,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let muted = cx.theme().muted_foreground;

    let ready_header = match &active_objects {
        ActiveObjects::Ready { label, engine, .. } => Some(
            h_flex()
                .px(px(SIDEBAR_INSET))
                .py_2()
                .gap_2()
                .items_center()
                .child(crate::widgets::engine_label_inline(*engine, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child(label.clone()),
                ),
        ),
        ActiveObjects::Error { label, message } => Some(
            v_flex()
                .px(px(SIDEBAR_INSET))
                .py_2()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().danger_foreground)
                        .child(format!("Could not load {label}")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(super::notify::error_one_liner(message)),
                ),
        ),
        _ => None,
    };

    v_flex()
        .flex_1()
        .min_h_0()
        .child(
            h_flex()
                .h(gpui::px(32.0))
                .px(px(SIDEBAR_INSET))
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .child("Objects"),
                ),
        )
        .when_some(ready_header, |pane, header| pane.child(header))
        .child(
            List::new(&list)
                .flex_1()
                .min_h_0()
                .w_full()
                .search_placeholder("Search objects"),
        )
}
