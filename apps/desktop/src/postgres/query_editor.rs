// postgres::query_editor — run ad-hoc SQL against a pool.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{InputEvent, InputState},
    menu::PopupMenu,
    table::TableState,
    tooltip::Tooltip,
    v_flex,
};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::connection::ConnectionId;
use crate::postgres::mutations::execute_sql;
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::read_only_striped;
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::ui::{metadata_pill, panel_header};
use crate::widgets::virtual_table::RowDelegate;
use crate::workspace::{TabSpec, enqueue_open_tab, notify};

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
    conn_id: ConnectionId,
    sql_input: Entity<InputState>,
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
    show_history: bool,
}

impl QueryEditorPanel {
    pub fn new(
        pool: PgPool,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_initial(pool, conn_id, None, true, window, cx)
    }

    pub fn new_with_initial(
        pool: PgPool,
        conn_id: ConnectionId,
        initial_sql: Option<String>,
        auto_run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let result = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .cell_selectable(true)
        });
        let sql_text = initial_sql.unwrap_or_else(|| "SELECT 1 AS one;".to_string());
        let sql_input = new_sql_input(&sql_text, window, cx);
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            conn_id,
            sql_input: sql_input.clone(),
            result,
            status: QueryStatus::Idle,
            show_history: false,
        };
        cx.subscribe_in(&sql_input, window, |panel, _, event, _, cx| {
            if let InputEvent::PressEnter {
                secondary: true,
                shift: false,
            } = event
            {
                panel.run(cx);
            }
        })
        .detach();
        if auto_run && !sql_text.trim().is_empty() {
            cx.defer_in(window, |panel, _, cx| {
                panel.run(cx);
            });
        }
        panel
    }

    pub fn load_sql(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        set_sql_input(&self.sql_input, sql, window, cx);
        cx.notify();
    }

    pub fn current_sql(&self, cx: &App) -> String {
        sql_from_input(&self.sql_input, cx)
    }

    fn open_explain(&mut self, cx: &mut Context<Self>) {
        let sql = self.current_sql(cx);
        if sql.trim().is_empty() {
            return;
        }
        enqueue_open_tab(
            TabSpec::Explain {
                conn_id: self.conn_id.clone(),
                label: "explain".into(),
                sql,
            },
            cx,
        );
        cx.refresh_windows();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let sql_raw = self.current_sql(cx);
        if sql_raw.trim().is_empty() {
            return;
        }
        let vars = cx.global::<crate::project::ProjectVars>().vars.clone();
        let sql = crate::project::substitute(&sql_raw, &vars);
        let sql_executed = sql.clone();
        let conn_id = self.conn_id.clone();
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
                        cx.update_global(|store: &mut QueryStore, _| {
                            store.push_history(HistoryEntry {
                                conn_id: conn_id.clone(),
                                query: sql_executed,
                                ran_at: OffsetDateTime::now_utc(),
                                duration_ms: ms,
                                row_count: Some(row_count as u64),
                            });
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
        let err_fg = cx.theme().danger_foreground;

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
            .child(
                Button::new("pg-explain")
                    .ghost()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.open_explain(cx))),
            )
            .child(
                Button::new("pg-history-toggle")
                    .ghost()
                    .label(if self.show_history {
                        "Hide history"
                    } else {
                        "History"
                    })
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.show_history = !panel.show_history;
                        cx.notify();
                    })),
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

        let main_column = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(
                div()
                    .p_2()
                    .child(sql_editor::code_editor_area(
                        &self.sql_input,
                        is_error,
                        200.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(read_only_striped(&self.result)),
            );

        let editor_body =
            h_flex()
                .flex_1()
                .min_h(px(0.0))
                .child(main_column)
                .when(self.show_history, |row| {
                    row.child(
                        v_flex()
                            .w(px(260.0))
                            .min_h(px(0.0))
                            .border_l_1()
                            .border_color(border)
                            .bg(cx.theme().muted.opacity(0.08))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(border)
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Recent (this connection)"),
                            )
                            .children(
                                cx.global::<QueryStore>()
                                    .history_for(&self.conn_id)
                                    .into_iter()
                                    .take(20)
                                    .enumerate()
                                    .map(|(i, e)| {
                                        let preview: SharedString =
                                            e.query.chars().take(80).collect::<String>().into();
                                        let full_query = e.query.clone();
                                        div()
                                            .id(("pg-hist", i))
                                            .px_3()
                                            .py_2()
                                            .border_b_1()
                                            .border_color(border)
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(muted)
                                            .child(preview)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |panel, _, window, cx| {
                                                    set_sql_input(
                                                        &panel.sql_input,
                                                        &full_query,
                                                        window,
                                                        cx,
                                                    );
                                                    cx.notify();
                                                }),
                                            )
                                    }),
                            ),
                    )
                });

        v_flex()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(cx.theme().background)
            .child(panel_header(
                "Postgres Query",
                "Run SQL, inspect plans, compare result sets",
                cx,
            ))
            .child(toolbar)
            .when_some(error_strip, |col, strip| col.child(strip))
            .child(editor_body)
    }
}
