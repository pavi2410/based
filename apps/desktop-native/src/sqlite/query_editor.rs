// sqlite::query_editor — QueryEditorPanel: run arbitrary SQL and view results.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
    table::{Column, DataTable, TableState},
};
use sqlx::{Column as SqlxColumn, Row, SqlitePool};

use crate::tokio_bridge;
use crate::widgets::virtual_table::RowDelegate;

pub enum QueryStatus {
    Idle,
    Running,
    Done { rows: usize, elapsed_ms: u64 },
    Error(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    sql: String,
    result_table: Option<Entity<TableState<RowDelegate>>>,
    status: QueryStatus,
}

impl QueryEditorPanel {
    pub fn new(pool: SqlitePool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql: String::from("SELECT * FROM sqlite_master LIMIT 20"),
            result_table: None,
            status: QueryStatus::Idle,
        }
    }

    fn run_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql = self.sql.clone();
        self.status = QueryStatus::Running;

        let delegate = RowDelegate::default();
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .cell_selectable(true)
        });
        self.result_table = Some(table.clone());

        cx.spawn(async move |this, cx| {
            let start = std::time::Instant::now();

            let result: anyhow::Result<(Vec<Column>, Vec<Vec<SharedString>>)> = tokio_bridge::block_on_db(
                async move {
                    let rows = sqlx::query(&sql).fetch_all(&pool).await?;
                    let columns: Vec<Column> = if let Some(first) = rows.first() {
                        first
                            .columns()
                            .iter()
                            .map(|c| Column::new(c.name().to_string(), c.name().to_string()))
                            .collect()
                    } else {
                        vec![]
                    };
                    let data_rows: Vec<Vec<SharedString>> = rows
                        .iter()
                        .map(|row| {
                            (0..row.len())
                                .map(|i| {
                                    let val: Option<String> = row.try_get(i).ok();
                                    SharedString::from(val.unwrap_or_default())
                                })
                                .collect()
                        })
                        .collect();
                    Ok((columns, data_rows))
                },
            );

            let elapsed_ms = start.elapsed().as_millis() as u64;

            cx.update(|cx| {
                this.update(cx, |panel, cx| match result {
                    Ok((columns, data_rows)) => {
                        let row_count = data_rows.len();
                        if let Some(ref tbl) = panel.result_table {
                            tbl.update(cx, |state, cx| {
                                let delegate = state.delegate_mut();
                                delegate.columns = columns;
                                delegate.rows = data_rows;
                                cx.notify();
                            });
                        }
                        panel.status = QueryStatus::Done {
                            rows: row_count,
                            elapsed_ms,
                        };
                        cx.notify();
                    }
                    Err(e) => {
                        panel.status = QueryStatus::Error(e.to_string());
                        cx.notify();
                    }
                })
            })
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for QueryEditorPanel {}

impl Focusable for QueryEditorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for QueryEditorPanel {
    fn panel_name(&self) -> &'static str {
        "SqliteQueryEditor"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Query Editor"
    }
}

impl Render for QueryEditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_text: SharedString = match &self.status {
            QueryStatus::Idle => "Ready".into(),
            QueryStatus::Running => "Running…".into(),
            QueryStatus::Done { rows, elapsed_ms } => {
                format!("{rows} rows in {elapsed_ms}ms").into()
            }
            QueryStatus::Error(e) => format!("Error: {e}").into(),
        };

        let is_error = matches!(self.status, QueryStatus::Error(_));
        let sql_text: SharedString = self.sql.clone().into();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let editor_placeholder = div()
            .id("sql-editor")
            .w_full()
            .h(px(160.0))
            .p(px(8.0))
            .border_1()
            .border_color(border)
            .rounded(px(4.0))
            .font_family("monospace")
            .text_sm()
            .child(sql_text);

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(4.0))
            .gap(px(8.0))
            .border_b_1()
            .border_color(border)
            .child(
                Button::new("run")
                    .label("Run (⌘↵)")
                    .on_click(cx.listener(|panel, _, window, cx| panel.run_query(window, cx))),
            )
            .child(
                div()
                    .text_sm()
                    .when(is_error, |d| d.text_color(rgb(0xff5555)))
                    .when(!is_error, |d| d.text_color(muted))
                    .child(status_text),
            );

        let bottom: AnyElement = if let Some(ref table) = self.result_table {
            div()
                .flex_1()
                .child(DataTable::new(table).stripe(true).bordered(false))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("No results yet")
                .into_any_element()
        };

        v_flex()
            .w_full()
            .h_full()
            .p(px(8.0))
            .gap(px(8.0))
            .child(editor_placeholder)
            .child(toolbar)
            .child(bottom)
    }
}
