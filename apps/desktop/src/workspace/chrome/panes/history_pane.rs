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
use crate::widgets::query_panel_extras::{HistoryFilter, filtered_history};
use crate::workspace::TabSpec;
use crate::workspace::Workspace;
use crate::workspace::tabs::enqueue_open_tab;

pub fn render_history_pane(
    workspace: Entity<Workspace>,
    conn_id: Option<ConnectionId>,
    filter: HistoryFilter,
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
        .child(filter_row);

    let rows = entries.into_iter().enumerate().map(|(i, e)| {
        let preview: SharedString = e.query.chars().take(120).collect::<String>().into();
        let full_query = e.query.clone();
        let conn_for_open = conn_id.clone();
        div()
            .id(SharedString::from(format!("ws-hist-{i}")))
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(border)
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
            })
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
