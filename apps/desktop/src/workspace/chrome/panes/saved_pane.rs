//! Workspace-level saved project queries pane.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, div, prelude::*,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::query_store::QueryStore;
use crate::workspace::Workspace;
use crate::workspace::project_query::{OpenQueryResult, open_project_query, target_hint};
use crate::workspace::tab_open::enqueue_open_tab;

pub fn render_saved_pane(
    conn_id: Option<ConnectionId>,
    registry: Entity<ConnectionRegistry>,
    workspace: Entity<Workspace>,
    cx: &mut App,
) -> AnyElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;

    let store = cx.global::<QueryStore>();
    let mut queries: Vec<_> = store.project_queries().to_vec();
    queries.sort_by(|a, b| {
        let af = store.is_favorite(&a.path);
        let bf = store.is_favorite(&b.path);
        bf.cmp(&af).then_with(|| a.name.cmp(&b.name))
    });

    if queries.is_empty() {
        return v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("No queries in this project."),
            )
            .into_any_element();
    }

    let conn_for_queries = conn_id.clone();
    let reg = registry.clone();
    let ws = workspace.clone();

    v_flex()
        .id("ws-saved-list")
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .children(queries.into_iter().enumerate().map(|(i, q)| {
            let title: SharedString = q.name.clone().into();
            let hint = target_hint(&q.target);
            let sub: SharedString = hint.into();
            let starred = store.is_favorite(&q.path);
            let path = q.path.clone();
            let query = q.clone();
            let reg = reg.clone();
            let ws = ws.clone();
            let conn_for_row = conn_for_queries.clone();
            v_flex()
                .id(SharedString::from(format!("ws-saved-{i}")))
                .px_3()
                .py_2()
                .gap_1()
                .border_b_1()
                .border_color(border)
                .cursor_pointer()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .truncate()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if starred { cx.theme().warning } else { muted })
                                .child(if starred { "★" } else { "☆" }),
                        ),
                )
                .child(div().text_xs().text_color(muted).truncate().child(sub))
                .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                    if ev.modifiers.shift {
                        if let Some(root) = cx
                            .try_global::<crate::project::ProjectRoot>()
                            .map(|p| p.0.clone())
                        {
                            cx.update_global(|store: &mut QueryStore, _| {
                                store.toggle_favorite(&root, &path);
                            });
                            ws.update(cx, |_, cx| cx.notify());
                        }
                        return;
                    }
                    match open_project_query(&query, reg.read(cx), cx, conn_for_row.as_ref()) {
                        OpenQueryResult::Open(spec) => enqueue_open_tab(spec, cx),
                        OpenQueryResult::PickConnection {
                            query_path,
                            candidates,
                        } => {
                            ws.update(cx, |ws, cx| {
                                ws.set_pending_target_pick(query.clone(), candidates);
                                cx.notify();
                            });
                            log::info!("ambiguous target for {query_path}; pick connection");
                        }
                        OpenQueryResult::Error(msg) => log::warn!("{msg}"),
                    }
                })
        }))
        .into_any_element()
}
