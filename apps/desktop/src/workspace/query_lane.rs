//! Loose queries and collections lane in the left sidebar.

use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use based_storage::MetadataStore;
use gpui_kit::component::{ActiveTheme, menu::ContextMenuExt, menu::PopupMenuItem, v_flex};
use gpui_kit::{
    AnyElement, App, FontWeight, MouseButton, ParentElement, SharedString, Styled, div, prelude::*,
};
use uuid::Uuid;

use crate::connection::ConnectionId;
use crate::connection::ConnectionState::Connected;
use crate::db;
use crate::storage;
use crate::widgets::section_eyebrow::section_eyebrow;
use crate::workspace::WorkspaceRef;
use crate::workspace::context::WorkspaceContext;
use crate::workspace::context::refresh_context;
use crate::workspace::tabs::enqueue_open_tab;
use crate::workspace::{QueryEditorInit, TabSpec};

pub fn render_query_lane(cx: &mut App) -> AnyElement {
    let ctx = cx
        .try_global::<WorkspaceContext>()
        .cloned()
        .unwrap_or_else(|| WorkspaceContext {
            active: based_workspace::WorkspaceModel::new("Default"),
            summaries: vec![],
        });
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;

    let mut body = v_flex()
        .id("query-lane")
        .w_full()
        .border_b_1()
        .border_color(border)
        .child(section_eyebrow("LOOSE QUERIES", cx));

    if ctx.active.loose_queries.is_empty() {
        body = body.child(
            div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(muted)
                .child("No loose queries yet."),
        );
    } else {
        for (i, q) in ctx.active.loose_queries.iter().enumerate() {
            let name: SharedString = q.name.clone().into();
            let sql = q.sql.clone();
            let query_id = q.id;
            let collection_names: Vec<String> = ctx
                .active
                .collections
                .iter()
                .map(|c| c.name.clone())
                .collect();
            body = body.child(
                div()
                    .id(SharedString::from(format!("loose-{i}")))
                    .px_3()
                    .py_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(border.opacity(0.5))
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted.opacity(0.25)))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .truncate()
                            .child(name),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        open_loose_query(&sql, cx);
                    })
                    .context_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for coll_name in &collection_names {
                            let coll_name = coll_name.clone();
                            menu = menu.item(
                                PopupMenuItem::new(format!("Move to {coll_name}")).on_click(
                                    move |_, _, cx| {
                                        move_loose_to_collection(query_id, &coll_name, cx);
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            );
        }
    }

    body = body
        .child(section_eyebrow("COLLECTIONS", cx))
        .child(new_actions(cx));

    if ctx.active.collections.is_empty() {
        body = body.child(
            div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(muted)
                .child("Create a collection to organize reusable SQL."),
        );
    } else {
        for (ci, coll) in ctx.active.collections.iter().enumerate() {
            body = body.child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(muted)
                    .child(coll.name.clone()),
            );
            if coll.queries.is_empty() {
                body = body.child(
                    div()
                        .px_3()
                        .pb_2()
                        .text_xs()
                        .text_color(muted.opacity(0.8))
                        .child("(empty)"),
                );
            }
            for (qi, q) in coll.queries.iter().enumerate() {
                let name: SharedString = q.name.clone().into();
                let sql = q.sql.clone();
                let query_id = q.id;
                body = body.child(
                    div()
                        .id(SharedString::from(format!("coll-{ci}-{qi}")))
                        .pl_5()
                        .pr_3()
                        .py_1()
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted.opacity(0.25)))
                        .child(div().text_xs().text_color(fg).truncate().child(name))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            open_loose_query(&sql, cx);
                        })
                        .context_menu(move |menu, _, _| {
                            menu.item(PopupMenuItem::new("Move to loose queries").on_click(
                                move |_, _, cx| {
                                    move_collection_to_loose(query_id, cx);
                                },
                            ))
                        }),
                );
            }
        }
    }

    body.into_any_element()
}

fn new_actions(cx: &mut App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    div()
        .px_3()
        .py_1()
        .flex()
        .gap_3()
        .text_xs()
        .text_color(muted)
        .child(
            div()
                .cursor_pointer()
                .hover(|s| s.text_color(cx.theme().foreground))
                .child("+ New query")
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    create_loose_query(cx);
                }),
        )
        .child(
            div()
                .cursor_pointer()
                .hover(|s| s.text_color(cx.theme().foreground))
                .child("+ Collection")
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    create_collection(cx);
                }),
        )
}

fn open_loose_query(sql: &str, cx: &mut App) {
    let conn_id = first_connected_conn_id(cx).unwrap_or_else(|| ConnectionId("pg".into()));
    enqueue_open_tab(
        TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit::Sql {
                sql: Some(sql.to_string()),
                auto_run: false,
            },
        },
        cx,
    );
}

fn first_connected_conn_id(cx: &App) -> Option<ConnectionId> {
    let ws = cx.try_global::<WorkspaceRef>()?;
    let registry = ws.0.read(cx).registry().clone();
    registry.read(cx).connections().iter().find_map(|e| {
        let entry = e.read(cx);
        if matches!(entry.state, Connected(_)) {
            Some(entry.id.clone())
        } else {
            None
        }
    })
}

pub fn create_loose_query_from_palette(cx: &mut App) {
    create_loose_query(cx);
}

pub fn create_collection_from_palette(cx: &mut App) {
    create_collection(cx);
}

/// Run a store mutation on Tokio, then refresh the workspace context without blocking the UI.
fn mutate<F, Fut>(cx: &mut App, op: F)
where
    F: FnOnce(Arc<MetadataStore>, WorkspaceContext) -> Fut + Send + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let Some(ctx) = cx.try_global::<WorkspaceContext>().cloned() else {
        return;
    };
    let store = storage::store(cx);
    let workspace_id = ctx.active.id;
    cx.spawn(async move |cx| {
        let refreshed = db::run(cx, async move {
            op(store.clone(), ctx).await?;
            refresh_context(store, workspace_id).await
        })
        .await;
        if let Ok(ctx) = refreshed {
            cx.update(|cx| {
                cx.set_global(ctx);
                if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                    ws.update(cx, |_, cx| cx.notify());
                }
            });
        }
    })
    .detach();
}

fn create_loose_query(cx: &mut App) {
    mutate(cx, |store, ctx| async move {
        let name = format!("Query {}", ctx.active.loose_queries.len() + 1);
        store
            .create_loose_query(ctx.active.id, &name, "SELECT 1;", None)
            .await?;
        Ok(())
    });
}

fn create_collection(cx: &mut App) {
    mutate(cx, |store, ctx| async move {
        let name = format!("Collection {}", ctx.active.collections.len() + 1);
        store.create_collection(ctx.active.id, &name).await?;
        Ok(())
    });
}

fn move_loose_to_collection(query_id: Uuid, collection_name: &str, cx: &mut App) {
    let name = collection_name.to_string();
    mutate(cx, move |store, ctx| async move {
        let Some(c) = ctx.active.collections.iter().find(|c| c.name == name) else {
            return Ok(());
        };
        store
            .move_query_to_collection(ctx.active.id, query_id, c.id)
            .await?;
        Ok(())
    });
}

fn move_collection_to_loose(query_id: Uuid, cx: &mut App) {
    mutate(cx, move |store, ctx| async move {
        store.move_query_to_loose(ctx.active.id, query_id).await?;
        Ok(())
    });
}
