// postgres::query_editor — run ad-hoc SQL against a pool.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{DataTable, TableState},
    tooltip::Tooltip,
    v_flex,
};

use crate::postgres::mutations::execute_sql;
use crate::widgets::ui::{metadata_pill, panel_header};
use crate::widgets::virtual_table::RowDelegate;
use crate::workspace::notify;
use sqlx::PgPool;

pub enum QueryStatus {
    Idle,
    Running,
    Done {
        rows: usize,
        affected: u64,
        elapsed_ms: u64,
    },
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

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
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
        let err_border = cx.theme().danger;
        let err_fg = cx.theme().danger_foreground;
        let sql_val: SharedString = self.sql_text.clone().into();

        let is_error = matches!(self.status, QueryStatus::Error(_));
        let err_text = match &self.status {
            QueryStatus::Error(s) => Some(s.clone()),
            _ => None,
        };

        let status_line: SharedString = match &self.status {
            QueryStatus::Idle => "Ready.".into(),
            QueryStatus::Running => "Running…".into(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
            } => format!("{rows} rows — {affected} affected — {elapsed_ms} ms").into(),
            QueryStatus::Error(_) => "Query failed.".into(),
        };

        let toolbar = h_flex()
            .gap_2()
            .px_2()
            .py(px(6.0))
            .items_center()
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(
                Button::new("pg-run")
                    .primary()
                    .label("Run")
                    .on_click(cx.listener(|panel, _, _, cx| panel.run(cx))),
            )
            .child(metadata_pill(
                "shortcut",
                if cfg!(target_os = "macos") {
                    "⌘↵"
                } else {
                    "Ctrl Enter"
                },
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(if is_error { err_fg } else { muted })
                    .child(status_line),
            );

        let error_strip = err_text.map(|full| {
            let tip = full.clone();
            let line = notify::error_one_liner(&full);
            div()
                .id("pg-query-error-strip")
                .px_2()
                .pb_1()
                .text_xs()
                .text_color(err_fg)
                .truncate()
                .child(line)
                .tooltip(move |window, app| Tooltip::new(tip.clone()).build(window, app))
        });

        v_flex()
            .w_full()
            .h_full()
            .min_h_0()
            .bg(cx.theme().background)
            .child(panel_header(
                "Postgres Query",
                "Run SQL, inspect plans, compare result sets",
                cx,
            ))
            .child(toolbar)
            .when_some(error_strip, |col, strip| col.child(strip))
            .child({
                div().p_2().child(
                    div()
                        .h(px(180.0))
                        .p_2()
                        .border_1()
                        .rounded(px(7.0))
                        .bg(cx.theme().muted.opacity(0.14))
                        .border_color(if is_error { err_border } else { border })
                        .font_family("monospace")
                        .text_sm()
                        .child(sql_val),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(DataTable::new(&self.result).stripe(true).bordered(false)),
            )
    }
}
