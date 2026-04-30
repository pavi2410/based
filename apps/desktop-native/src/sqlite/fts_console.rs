// sqlite::fts_console — FtsConsolePanel: search across FTS5 tables.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};
use sqlx::{Row, SqlitePool};

pub struct FtsResult {
    pub table: String,
    pub snippet: String,
    pub rank: f64,
}

pub struct FtsConsolePanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    fts_tables: Vec<String>,
    selected_table: Option<String>,
    query: String,
    results: Vec<FtsResult>,
    no_fts: bool,
}

impl FtsConsolePanel {
    pub fn new(pool: SqlitePool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            fts_tables: vec![],
            selected_table: None,
            query: String::new(),
            results: vec![],
            no_fts: false,
        };
        panel.detect_fts_tables(cx);
        panel
    }

    fn detect_fts_tables(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let rows = match crate::db::run(cx, async move {
                Ok(
                    sqlx::query(
                        "SELECT name FROM sqlite_master WHERE type='table' AND sql LIKE '%fts5%'",
                    )
                    .fetch_all(&pool)
                    .await?,
                )
            }).await {
                Ok(r) => r,
                Err(_) => return,
            };

            let tables: Vec<String> = rows.iter().map(|r| r.get::<String, _>("name")).collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.no_fts = tables.is_empty();
                    panel.selected_table = tables.first().cloned();
                    panel.fts_tables = tables;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(table) = self.selected_table.clone() else {
            return;
        };
        let query = self.query.clone();
        let pool = self.pool.clone();

        cx.spawn(async move |this, cx| {
            let sql = format!(
                "SELECT snippet(\"{table}\", -1, '<b>', '</b>', '…', 32) AS snip, rank \
                 FROM \"{table}\" WHERE \"{table}\" MATCH ?1 ORDER BY rank"
            );
            let rows = match crate::db::run(cx, async move {
                Ok(sqlx::query(&sql).bind(&query).fetch_all(&pool).await?)
            }).await {
                Ok(r) => r,
                Err(_) => return,
            };

            let results: Vec<FtsResult> = rows
                .iter()
                .map(|row| FtsResult {
                    table: table.clone(),
                    snippet: row.try_get::<String, _>("snip").unwrap_or_default(),
                    rank: row.try_get::<f64, _>("rank").unwrap_or(0.0),
                })
                .collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.results = results;
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for FtsConsolePanel {}

impl Focusable for FtsConsolePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for FtsConsolePanel {
    fn panel_name(&self) -> &'static str {
        "SqliteFtsConsole"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "FTS Console"
    }
}

impl Render for FtsConsolePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let fg = cx.theme().foreground;

        if self.no_fts {
            return v_flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("No FTS5 tables found")
                .into_any_element();
        }

        let table_label: SharedString = self.selected_table.clone().unwrap_or_default().into();
        let query_text: SharedString = self.query.clone().into();

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(4.0))
            .gap(px(8.0))
            .border_b_1()
            .border_color(border)
            .child(div().text_sm().text_color(muted).child("Table: "))
            .child(div().text_sm().child(table_label))
            .child(
                div()
                    .id("fts-query")
                    .flex_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(4.0))
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_sm()
                    .child(query_text),
            )
            .child(
                Button::new("search")
                    .label("Search")
                    .on_click(cx.listener(|panel, _, _window, cx| panel.run_search(cx))),
            );

        let results: Vec<_> = self
            .results
            .iter()
            .map(|r| {
                let snip: SharedString = r.snippet.clone().into();
                div()
                    .w_full()
                    .py(px(4.0))
                    .px(px(8.0))
                    .border_b_1()
                    .border_color(border)
                    .text_sm()
                    .text_color(fg)
                    .child(snip)
            })
            .collect();

        v_flex()
            .w_full()
            .h_full()
            .child(toolbar)
            .child(
                v_flex()
                    .id("fts-results")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(results),
            )
            .into_any_element()
    }
}
