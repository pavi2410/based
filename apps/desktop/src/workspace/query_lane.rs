//! Loose queries and collections lane in the left sidebar.

use gpui::{
    AnyElement, App, FontWeight, MouseButton, ParentElement, SharedString, Styled, div, prelude::*,
};
use gpui_component::{ActiveTheme, menu::ContextMenuExt, v_flex};
use uuid::Uuid;

use crate::connection::ConnectionId;
use crate::storage;
use crate::workspace::context::WorkspaceContext;
use crate::workspace::tab_open::enqueue_open_tab;
use crate::workspace::tab_spec::TabSpec;
use crate::workspace::WorkspaceRef;

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
        .child(section_header("LOOSE QUERIES", cx));

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
                                gpui_component::menu::PopupMenuItem::new(format!(
                                    "Move to {coll_name}"
                                ))
                                .on_click(move |_, _, cx| {
                                    move_loose_to_collection(query_id, &coll_name, cx);
                                }),
                            );
                        }
                        menu
                    }),
            );
        }
    }

    body = body
        .child(section_header("COLLECTIONS", cx))
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
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg)
                                .truncate()
                                .child(name),
                        )
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            open_loose_query(&sql, cx);
                        })
                        .context_menu(move |menu, _, _| {
                            menu.item(
                                gpui_component::menu::PopupMenuItem::new("Move to loose queries")
                                    .on_click(move |_, _, cx| {
                                        move_collection_to_loose(query_id, cx);
                                    }),
                            )
                        }),
                );
            }
        }
    }

    body.into_any_element()
}

fn section_header(label: &str, cx: &App) -> impl IntoElement {
    div()
        .h_8()
        .px_3()
        .flex()
        .items_center()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label.to_string())
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

fn empty_hint(text: &str, cx: &App) -> AnyElement {
    v_flex()
        .px_3()
        .py_2()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}

fn open_loose_query(sql: &str, cx: &mut App) {
    let conn_id = first_connected_conn_id(cx).unwrap_or_else(|| ConnectionId("pg".into()));
    enqueue_open_tab(
        TabSpec::QueryEditor {
            conn_id,
            initial_sql: Some(sql.to_string()),
            initial_pipeline: None,
            auto_run: false,
            mongo_collection: None,
        },
        cx,
    );
}

fn first_connected_conn_id(cx: &App) -> Option<ConnectionId> {
    let ws = cx.try_global::<WorkspaceRef>()?;
    let registry = ws.0.read(cx).registry().clone();
    registry
        .read(cx)
        .connections()
        .iter()
        .find_map(|e| {
            let entry = e.read(cx);
            if matches!(entry.state, crate::connection::ConnectionState::Connected(_)) {
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

fn create_loose_query(cx: &mut App) {
    let Some(ctx) = cx.try_global::<WorkspaceContext>().cloned() else {
        return;
    };
    let store = storage::store(cx);
    let handle = gpui_tokio::Tokio::handle(cx);
    let name = format!("Query {}", ctx.active.loose_queries.len() + 1);
    let workspace_id = ctx.active.id;
    let result = handle.block_on(async move {
        store
            .create_loose_query(workspace_id, &name, "SELECT 1;", None)
            .await
    });
    if let Ok(_q) = result {
        reload_workspace_context(workspace_id, cx);
    }
}

fn create_collection(cx: &mut App) {
    let Some(ctx) = cx.try_global::<WorkspaceContext>().cloned() else {
        return;
    };
    let store = storage::store(cx);
    let handle = gpui_tokio::Tokio::handle(cx);
    let name = format!("Collection {}", ctx.active.collections.len() + 1);
    let workspace_id = ctx.active.id;
    let result = handle.block_on(async move {
        store.create_collection(workspace_id, &name).await
    });
    if result.is_ok() {
        reload_workspace_context(workspace_id, cx);
    }
}

fn move_loose_to_collection(query_id: Uuid, collection_name: &str, cx: &mut App) {
    let Some(ctx) = cx.try_global::<WorkspaceContext>().cloned() else {
        return;
    };
    let Some(collection) = ctx
        .active
        .collections
        .iter()
        .find(|c| c.name == collection_name)
    else {
        return;
    };
    let store = storage::store(cx);
    let handle = gpui_tokio::Tokio::handle(cx);
    let workspace_id = ctx.active.id;
    let collection_id = collection.id;
    let result = handle.block_on(async move {
        store
            .move_query_to_collection(workspace_id, query_id, collection_id)
            .await
    });
    if result.is_ok() {
        reload_workspace_context(workspace_id, cx);
    }
}

fn move_collection_to_loose(query_id: Uuid, cx: &mut App) {
    let Some(ctx) = cx.try_global::<WorkspaceContext>().cloned() else {
        return;
    };
    let store = storage::store(cx);
    let handle = gpui_tokio::Tokio::handle(cx);
    let workspace_id = ctx.active.id;
    let result = handle
        .block_on(async move { store.move_query_to_loose(workspace_id, query_id).await });
    if result.is_ok() {
        reload_workspace_context(workspace_id, cx);
    }
}

pub fn reload_workspace_context(workspace_id: Uuid, cx: &mut App) {
    let store = storage::store(cx);
    let handle = gpui_tokio::Tokio::handle(cx);
    if let Ok(ctx) = handle.block_on(crate::workspace::context::refresh_context(store, workspace_id))
    {
        cx.set_global(ctx);
        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
            ws.update(cx, |_, cx| cx.notify());
        }
    }
}
