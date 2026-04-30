// postgres::query_editor — run ad-hoc SQL against a pool.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
    table::{DataTable, TableState},
};

use crate::postgres::mutations::execute_sql;
use crate::widgets::virtual_table::RowDelegate;
use sqlx::PgPool;

pub enum QueryStatus {
    Idle,
    Running,
    Done { rows: usize, affected: u64, elapsed_ms: u64 },
    Error(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    sql_text: String,
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
}

impl QueryEditorPanel {
    pub fn new(pool: PgPool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = RowDelegate::default();
        let result = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .cell_selectable(true)
        });
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql_text: String::from("SELECT 1 AS one;"),
            result,
            status: QueryStatus::Idle,
        };
        cx.defer_in(window, |panel, _, cx| {
            panel.run(cx);
        });
        panel
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let sql = self.sql_text.clone();
        if sql.trim().is_empty() {
            return;
        }
        self.status = QueryStatus::Running;
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let start = std::time::Instant::now();
            let outcome = crate::db::run(cx, async move { execute_sql(&pool, &sql).await }).await;
            let ms = start.elapsed().as_millis() as u64;
            let _ = this.update(cx, |panel, cx| {
                panel.status = match outcome {
                    Ok((cols, rows, aff)) => {
                        let col_models: Vec<gpui_component::table::Column> = cols
                            .into_iter()
                            .map(|c| gpui_component::table::Column::new(c.clone(), c))
                            .collect();
                        let data: Vec<Vec<SharedString>> = rows
                            .into_iter()
                            .map(|r| r.into_iter().map(SharedString::from).collect())
                            .collect();
                        let row_count = data.len();
                        panel.result.update(cx, |state, cx| {
                            let d = state.delegate_mut();
                            d.columns = col_models;
                            d.rows = data;
                            cx.notify();
                        });
                        QueryStatus::Done {
                            rows: row_count,
                            affected: aff,
                            elapsed_ms: ms,
                        }
                    }
                    Err(e) => QueryStatus::Error(e.to_string()),
                };
                cx.notify();
            });
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
        "PgQueryEditor"
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
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let sql_val: SharedString = self.sql_text.clone().into();

        let status_line: SharedString = match &self.status {
            QueryStatus::Idle => "Ready.".into(),
            QueryStatus::Running => "Running…".into(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
            } => format!("{rows} rows — {affected} affected — {elapsed_ms} ms").into(),
            QueryStatus::Error(e) => format!("Error: {e}").into(),
        };

        v_flex()
            .w_full()
            .h_full()
            .min_h_0()
            .child(
                div()
                    .flex_1()
                    .min_h(px(120.0))
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .font_family("monospace")
                    .text_sm()
                    .child(sql_val),
            )
            .child(
                h_flex()
                    .gap_2()
                    .p_2()
                    .items_center()
                    .child(
                        Button::new("pg-run")
                            .primary()
                            .label("Run")
                            .on_click(cx.listener(|panel, _, _, cx| panel.run(cx))),
                    )
                    .child(div().text_sm().text_color(muted).child(status_line)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(DataTable::new(&self.result).stripe(true).bordered(false)),
            )
    }
}
