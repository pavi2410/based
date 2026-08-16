//! Catalog / Queries split for the focused connection.

use gpui::{
    App, Entity, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, WeakEntity,
    div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    list::List,
    list::ListState,
    v_flex,
};

use crate::query_store::QueryStore;
use crate::widgets::SIDEBAR_INSET;
use crate::widgets::empty_state::pane_empty_hint;
use crate::workspace::notify;
use crate::workspace::project_query::{
    OpenQueryResult, open_project_query, query_targets_connection, target_hint,
};

use super::ConnectionTree;
use super::TreeEvent;
use super::browser_list::BrowserListDelegate;

pub(crate) fn render_content_rail(
    tree: &ConnectionTree,
    tree_entity: WeakEntity<ConnectionTree>,
    catalog_list: Entity<ListState<BrowserListDelegate>>,
    cx: &mut App,
) -> impl IntoElement {
    let catalog_count = catalog_list.read(cx).delegate().object_count();
    let catalog_open = tree.catalog_search_open;
    let queries_open = tree.queries_search_open;
    let catalog_collapsed = tree.catalog_collapsed;
    let queries_collapsed = tree.queries_collapsed;
    let catalog_input = tree.catalog_search.clone();
    let queries_input = tree.queries_search.clone();
    let conn_id = tree.selected_connection_id(cx);
    let query_filter = queries_input
        .as_ref()
        .map(|input| input.read(cx).value().trim().to_ascii_lowercase())
        .unwrap_or_default();

    let mut queries = Vec::new();
    if let Some(conn_id) = conn_id.clone() {
        let registry = tree.registry.read(cx);
        let store = cx.global::<QueryStore>();
        for query in store.project_queries() {
            if !query_targets_connection(query, &conn_id, registry, cx) {
                continue;
            }
            if !query_filter.is_empty() {
                let name_hit = query.name.to_ascii_lowercase().contains(&query_filter);
                let path_hit = query.path.to_ascii_lowercase().contains(&query_filter);
                let hint_hit = target_hint(&query.target)
                    .to_ascii_lowercase()
                    .contains(&query_filter);
                if !name_hit && !path_hit && !hint_hit {
                    continue;
                }
            }
            queries.push(query.clone());
        }
    }
    let query_count = queries.len();

    v_flex()
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .child(pane_shell(
            "Catalog",
            catalog_count,
            catalog_open,
            catalog_collapsed,
            catalog_input,
            tree_entity.clone(),
            true,
            cx,
            List::new(&catalog_list)
                .flex_1()
                .min_h_0()
                .w_full()
                .into_any_element(),
        ))
        .child(pane_shell(
            "Queries",
            query_count,
            queries_open,
            queries_collapsed,
            queries_input,
            tree_entity.clone(),
            false,
            cx,
            render_queries_list(queries, tree_entity, cx),
        ))
}

#[allow(clippy::too_many_arguments)]
fn pane_shell(
    title: &'static str,
    count: usize,
    search_open: bool,
    collapsed: bool,
    search_input: Option<Entity<InputState>>,
    tree: WeakEntity<ConnectionTree>,
    is_catalog: bool,
    cx: &App,
    body: impl IntoElement,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let muted = cx.theme().muted_foreground;
    let tree_toggle = tree.clone();
    let tree_collapse = tree;

    v_flex()
        .when(collapsed, |pane| pane.flex_none())
        .when(!collapsed, |pane| pane.flex_1().min_h(px(80.0)).min_h_0())
        .when(!is_catalog, |pane| pane.border_t_1().border_color(border))
        .child(
            h_flex()
                .h(px(32.0))
                .px(px(SIDEBAR_INSET))
                .gap_2()
                .items_center()
                .cursor_pointer()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    if let Some(ent) = tree_collapse.upgrade() {
                        ent.update(cx, |t, cx| t.toggle_pane_collapsed(is_catalog, cx));
                    }
                })
                .child(
                    Icon::new(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall()
                    .text_color(muted),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted.opacity(0.8))
                        .child(count.to_string()),
                )
                .child(div().flex_1())
                .child(
                    Button::new(if is_catalog {
                        "catalog-search-toggle"
                    } else {
                        "queries-search-toggle"
                    })
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(if search_open {
                        IconName::Close
                    } else {
                        IconName::Search
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, window, cx| {
                        if let Some(ent) = tree_toggle.upgrade() {
                            ent.update(cx, |t, cx| {
                                t.toggle_pane_search(is_catalog, window, cx);
                            });
                        }
                    }),
                ),
        )
        .when(!collapsed && search_open, |pane| {
            pane.when_some(search_input, |pane, input| {
                pane.child(
                    div()
                        .px(px(SIDEBAR_INSET))
                        .py(px(4.0))
                        .border_b_1()
                        .border_color(border.opacity(0.7))
                        .child(Input::new(&input).appearance(false).cleanable(true)),
                )
            })
        })
        .when(!collapsed, |pane| pane.child(body))
}

fn render_queries_list(
    queries: Vec<based_project::ProjectQuery>,
    tree: WeakEntity<ConnectionTree>,
    cx: &App,
) -> impl IntoElement {
    if queries.is_empty() {
        return pane_empty_hint("No saved queries", cx).into_any_element();
    }

    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;

    v_flex()
        .id("connection-queries")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .children(queries.into_iter().enumerate().map(|(i, query)| {
            let title: SharedString = query.name.clone().into();
            let hint: SharedString = target_hint(&query.target).into();
            let tree = tree.clone();
            v_flex()
                .id(SharedString::from(format!("conn-query-{i}")))
                .px(px(SIDEBAR_INSET))
                .py_2()
                .gap_1()
                .border_b_1()
                .border_color(border)
                .cursor_pointer()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .truncate()
                        .child(title),
                )
                .child(div().text_xs().text_color(muted).truncate().child(hint))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let Some(ent) = tree.upgrade() else {
                        return;
                    };
                    ent.update(cx, |tree, cx| {
                        let Some(conn_id) = tree.selected_connection_id(cx) else {
                            return;
                        };
                        match open_project_query(&query, tree.registry.read(cx), cx, Some(&conn_id))
                        {
                            OpenQueryResult::Open(spec) => cx.emit(TreeEvent::OpenTab(spec)),
                            OpenQueryResult::PickConnection { query_path, .. } => {
                                notify::push_info(
                                    cx,
                                    format!("Pick a connection for {query_path}"),
                                );
                            }
                            OpenQueryResult::Error(msg) => notify::push_error(cx, "Query", msg),
                        }
                    });
                })
        }))
        .into_any_element()
}
