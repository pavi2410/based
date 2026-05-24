use gpui::{Context, FontWeight, IntoElement, ParentElement, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, h_flex, list::List, v_flex};

use crate::widgets::ui::{SIDEBAR_INSET, engine_label_inline};

use super::ConnectionTree;
use super::object_list::{ensure_object_list, refresh_object_list};
use super::types::ActiveObjects;

pub(super) fn render_objects_pane(
    active_objects: ActiveObjects,
    tree: &mut ConnectionTree,
    window: &mut Window,
    cx: &mut Context<ConnectionTree>,
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
                .child(engine_label_inline(*engine, cx))
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

    let list = ensure_object_list(tree, window, cx);
    refresh_object_list(tree, cx);

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
