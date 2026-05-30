//! Workspace-level saved (starred) queries pane. Clicking a row opens a new
//! Query tab seeded with the saved SQL / pipeline.

use gpui::{
    AnyElement, App, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, Styled,
    div, prelude::*,
};
use gpui_component::{ActiveTheme, v_flex};

use crate::connection::ConnectionId;
use crate::query_store::{QueryStore, SavedQuery};
use crate::workspace::tab_open::enqueue_open_tab;
use crate::workspace::tab_spec::TabSpec;

pub fn render_saved_pane(conn_id: Option<ConnectionId>, cx: &mut App) -> AnyElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;

    let queries: Vec<SavedQuery> = {
        let store = cx.global::<QueryStore>();
        let all = store.all_saved();
        match &conn_id {
            Some(id) => all
                .iter()
                .filter(|q| &q.connection == id)
                .cloned()
                .collect(),
            None => all.to_vec(),
        }
    };

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
                    .child("No saved queries. Star a row in History to save it."),
            )
            .into_any_element();
    }

    v_flex()
        .id("ws-saved-list")
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .children(queries.into_iter().enumerate().map(|(i, q)| {
            let title: SharedString = q.name.clone().into();
            let preview: SharedString = q.query_text().chars().take(100).collect::<String>().into();
            let conn = q.connection.clone();
            let sql_text = q.query_text().to_string();
            let is_mongo = q.pipeline.is_some();
            let mongo_collection = q.mongo_collection.clone();
            v_flex()
                .id(SharedString::from(format!("ws-saved-{i}")))
                .px_3()
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
                .child(
                    div()
                        .text_xs()
                        .font_family(crate::app::prefs::code_font_family(cx))
                        .text_color(muted)
                        .child(preview),
                )
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let spec = if is_mongo {
                        TabSpec::QueryEditor {
                            conn_id: conn.clone(),
                            initial_sql: None,
                            initial_pipeline: Some(sql_text.clone()),
                            auto_run: false,
                            mongo_collection: mongo_collection.clone(),
                        }
                    } else {
                        TabSpec::QueryEditor {
                            conn_id: conn.clone(),
                            initial_sql: Some(sql_text.clone()),
                            initial_pipeline: None,
                            auto_run: false,
                            mongo_collection: None,
                        }
                    };
                    enqueue_open_tab(spec, cx);
                })
        }))
        .into_any_element()
}
