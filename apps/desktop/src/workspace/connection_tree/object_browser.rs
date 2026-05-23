use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};

use crate::connection::{ConnectionId, EngineKind};
use crate::widgets::list_row::schema_object_row_with_actions;
use crate::widgets::ui::engine_chip;

use super::types::{ActiveObjects, ObjectKind, SchemaObject};
use super::{ConnectionTree, notify};

pub(super) fn render_objects_pane(
    active_objects: ActiveObjects,
    selected_object: Option<String>,
    conn_id_for_tabs: Option<ConnectionId>,
    cx: &mut Context<ConnectionTree>,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let muted = cx.theme().muted_foreground;
    let sfg = cx.theme().sidebar_foreground;
    let mono = cx.theme().mono_font_family.clone();

    let body: AnyElement = match active_objects {
        ActiveObjects::Empty => v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_3()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Select a connected database to browse objects."),
            )
            .into_any_element(),
        ActiveObjects::Loading { label, engine } => v_flex()
            .flex_1()
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(engine_chip(engine, cx))
                    .child(div().text_xs().text_color(muted).truncate().child(label)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Loading objects..."),
            )
            .into_any_element(),
        ActiveObjects::Error { label, message } => v_flex()
            .flex_1()
            .p_3()
            .gap_2()
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
                    .child(notify::error_one_liner(&message)),
            )
            .into_any_element(),
        ActiveObjects::Ready {
            label,
            engine,
            objects,
        } => {
            let mut groups: Vec<(&'static str, Vec<SchemaObject>)> = Vec::new();
            for object in objects {
                let group = object.kind.group();
                if let Some((_, rows)) = groups.iter_mut().find(|(name, _)| *name == group) {
                    rows.push(object);
                } else {
                    groups.push((group, vec![object]));
                }
            }

            v_flex()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(
                    h_flex()
                        .px_2()
                        .py_2()
                        .gap_2()
                        .items_center()
                        .child(engine_chip(engine, cx))
                        .child(div().text_xs().text_color(muted).truncate().child(label)),
                )
                .children(groups.into_iter().map(|(group_name, rows)| {
                    v_flex()
                        .mb(gpui::px(4.0))
                        .child(
                            h_flex()
                                .px_2()
                                .py_1()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_bold()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(muted.opacity(0.86))
                                        .child(group_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(muted.opacity(0.76))
                                        .child(rows.len().to_string()),
                                ),
                        )
                        .children(rows.into_iter().map(|object| {
                            let object_id = object.display_name();
                            let mut hasher = DefaultHasher::new();
                            object_id.hash(&mut hasher);
                            object.kind.label().hash(&mut hasher);
                            let object_key = hasher.finish();
                            let is_selected =
                                selected_object.as_deref() == Some(object_id.as_str());
                            let kind: SharedString = object.kind.badge_label().into();
                            let object_id_label: SharedString = object_id.clone().into();
                            let object_for_row_click = object.clone();

                            let show_inspect = conn_id_for_tabs.is_some()
                                && matches!(
                                    object.kind,
                                    ObjectKind::Table
                                        | ObjectKind::View
                                        | ObjectKind::MaterializedView
                                        | ObjectKind::Collection
                                );
                            let show_insert = conn_id_for_tabs.is_some()
                                && matches!(engine, EngineKind::MongoDB)
                                && matches!(object.kind, ObjectKind::Collection);

                            let mut actions = h_flex().gap_1();
                            if show_inspect {
                                let cid = conn_id_for_tabs.clone().unwrap();
                                let o = object.clone();
                                actions = actions.child(
                                    Button::new(("obj-inspect", object_key))
                                        .small()
                                        .ghost()
                                        .label("◇")
                                        .on_click(cx.listener(move |tree, _, _, cx| {
                                            cx.stop_propagation();
                                            tree.open_inspector_tab(o.clone(), cid.clone(), cx);
                                        })),
                                );
                            }
                            if show_insert {
                                let cid = conn_id_for_tabs.clone().unwrap();
                                let o = object.clone();
                                actions = actions.child(
                                    Button::new(("obj-insert", object_key))
                                        .small()
                                        .ghost()
                                        .label("+")
                                        .on_click(cx.listener(move |tree, _, _, cx| {
                                            cx.stop_propagation();
                                            tree.open_document_insert_tab(
                                                o.clone(),
                                                cid.clone(),
                                                cx,
                                            );
                                        })),
                                );
                            }

                            schema_object_row_with_actions(
                                ("object-row", object_key),
                                is_selected,
                                kind,
                                object_id_label,
                                muted,
                                sfg,
                                mono.clone(),
                                actions,
                            )
                            .mx_2()
                            .mb(gpui::px(1.0))
                            .rounded(gpui::px(6.0))
                            .on_click(cx.listener(move |tree, _, window, cx| {
                                tree.on_object_clicked(object_for_row_click.clone(), window, cx);
                            }))
                        }))
                }))
                .into_any_element()
        }
    };

    v_flex()
        .flex_1()
        .min_h_0()
        .child(
            h_flex()
                .h(gpui::px(38.0))
                .px_2()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_bold()
                        .text_color(muted)
                        .font_family(cx.theme().mono_font_family.clone())
                        .child("OBJECTS"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted.opacity(0.78))
                        .child("connection scoped"),
                ),
        )
        .child(
            h_flex()
                .mx_2()
                .my_2()
                .h(gpui::px(28.0))
                .items_center()
                .gap_2()
                .px_2()
                .rounded(gpui::px(6.0))
                .border_1()
                .border_color(border.opacity(0.78))
                .bg(cx.theme().muted.opacity(0.32))
                .child(
                    Icon::new(IconName::Search)
                        .with_size(gpui_component::Size::XSmall)
                        .text_color(muted),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child("Search objects"),
                ),
        )
        .child(body)
}
