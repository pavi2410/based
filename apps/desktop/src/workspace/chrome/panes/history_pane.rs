//! Cross-engine query history pane shown in the workspace right-hand column.

use gpui::{
    AnyElement, App, Entity, IntoElement, MouseButton, ParentElement, SharedString, Styled, div,
    prelude::*,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::connection::ConnectionId;
use crate::query_store::QueryStore;
use crate::widgets::query_panel_extras::{HistoryFilter, filtered_history, save_starred_query};
use crate::workspace::Workspace;
use crate::workspace::tab_open::enqueue_open_tab;
use crate::workspace::tab_spec::TabSpec;

pub fn render_history_pane(
    workspace: Entity<Workspace>,
    conn_id: Option<ConnectionId>,
    filter: HistoryFilter,
    pending_star: Option<(String, String)>,
    cx: &mut App,
) -> AnyElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    let Some(conn_id) = conn_id else {
        return v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Open a Query tab to see its run history."),
            )
            .into_any_element();
    };

    let entries = {
        let store = cx.global::<QueryStore>();
        filtered_history(store, &conn_id, filter)
    };

    let filter_row = h_flex().gap_1().children(HistoryFilter::ALL.map(|f| {
        let active = filter == f;
        let workspace = workspace.clone();
        Button::new(SharedString::from(format!("ws-hist-filter-{}", f.label())))
            .ghost()
            .xsmall()
            .label(f.label())
            .when(active, |b| b.primary())
            .on_click(move |_, _, cx| {
                let ws = workspace.clone();
                ws.update(cx, |w, cx| {
                    w.set_history_filter(f, cx);
                });
            })
    }));

    let header_section = v_flex()
        .px_3()
        .py_2()
        .gap_2()
        .border_b_1()
        .border_color(border)
        .flex_shrink_0()
        .child(filter_row)
        .when_some(pending_star, |col, (query, name)| {
            let ws_save = workspace.clone();
            let ws_cancel = workspace.clone();
            let conn_for_save = conn_id.clone();
            let query_for_save = query.clone();
            let name_for_save = name.clone();
            let name_display = name.clone();
            col.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Save as \"{name_display}\"")),
                    )
                    .child(
                        Button::new("ws-star-save")
                            .primary()
                            .xsmall()
                            .label("Save")
                            .on_click(move |_, _, cx| {
                                let conn = conn_for_save.clone();
                                let name = name_for_save.clone();
                                let q = query_for_save.clone();
                                cx.update_global(|store: &mut QueryStore, _| {
                                    save_starred_query(store, conn, &name, &q, false, None);
                                });
                                let ws = ws_save.clone();
                                ws.update(cx, |w, cx| w.clear_pending_star(cx));
                            }),
                    )
                    .child(
                        Button::new("ws-star-cancel")
                            .ghost()
                            .xsmall()
                            .label("Cancel")
                            .on_click(move |_, _, cx| {
                                let ws = ws_cancel.clone();
                                ws.update(cx, |w, cx| w.clear_pending_star(cx));
                            }),
                    ),
            )
        });

    let rows = entries.into_iter().enumerate().map(|(i, e)| {
        let preview: SharedString = e.query.chars().take(120).collect::<String>().into();
        let full_query = e.query.clone();
        let full_for_star = full_query.clone();
        let conn_for_open = conn_id.clone();
        let workspace_star = workspace.clone();
        h_flex()
            .id(SharedString::from(format!("ws-hist-{i}")))
            .px_3()
            .py_2()
            .gap_1()
            .border_b_1()
            .border_color(border)
            .items_start()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .text_xs()
                    .font_family(crate::app::prefs::code_font_family(cx))
                    .text_color(muted)
                    .child(preview)
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        enqueue_open_tab(
                            TabSpec::QueryEditor {
                                conn_id: conn_for_open.clone(),
                                initial_sql: Some(full_query.clone()),
                                initial_pipeline: None,
                                auto_run: false,
                                mongo_collection: None,
                            },
                            cx,
                        );
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("ws-star-{i}")))
                    .ghost()
                    .xsmall()
                    .label("★")
                    .on_click(move |_, _, cx| {
                        let ws = workspace_star.clone();
                        let q = full_for_star.clone();
                        let name = format!("query_{i}");
                        ws.update(cx, |w, cx| w.set_pending_star(q, name, cx));
                    }),
            )
    });

    v_flex()
        .size_full()
        .min_h_0()
        .child(header_section)
        .child(
            v_flex()
                .id("ws-history-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(rows),
        )
        .into_any_element()
}
