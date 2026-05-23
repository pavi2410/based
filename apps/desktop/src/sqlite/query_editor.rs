// sqlite::query_editor — QueryEditorPanel: run arbitrary SQL and view results.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{InputEvent, InputState},
    menu::PopupMenu,
    table::{Column, TableState},
    tooltip::Tooltip,
    v_flex,
};
use sqlx::{Column as SqlxColumn, Row, SqlitePool};
use time::OffsetDateTime;

use crate::connection::ConnectionId;
use crate::db;
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::read_only_striped;
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::ui::{metadata_pill, panel_header};
use crate::widgets::virtual_table::RowDelegate;
use crate::workspace::notify;

pub enum QueryStatus {
    Idle,
    Running,
    Done { rows: usize, elapsed_ms: u64 },
    Error(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    conn_id: ConnectionId,
    sql_input: Entity<InputState>,
    result_table: Option<Entity<TableState<RowDelegate>>>,
    status: QueryStatus,
    show_history: bool,
}

impl QueryEditorPanel {
    pub fn new(
        pool: SqlitePool,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_initial(pool, conn_id, None, true, window, cx)
    }

    pub fn new_with_initial(
        pool: SqlitePool,
        conn_id: ConnectionId,
        initial_sql: Option<String>,
        auto_run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sql = initial_sql.unwrap_or_else(|| "SELECT * FROM sqlite_master LIMIT 20".to_string());
        let sql_input = new_sql_input(&sql, window, cx);
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            conn_id,
            sql_input: sql_input.clone(),
            result_table: None,
            status: QueryStatus::Idle,
            show_history: false,
        };
        cx.subscribe_in(&sql_input, window, |panel, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: true,
                shift: false,
            } = event
            {
                panel.run_query(window, cx);
            }
        })
        .detach();
        if auto_run && !sql.trim().is_empty() {
            cx.defer_in(window, |panel, window, cx| {
                panel.run_query(window, cx);
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

    fn run_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql_raw = self.current_sql(cx);
        let vars = cx.global::<crate::project::ProjectVars>().vars.clone();
        let sql = crate::project::substitute(&sql_raw, &vars);
        let sql_executed = sql.clone();
        let conn_id = self.conn_id.clone();
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

            let result: anyhow::Result<(Vec<Column>, Vec<Vec<SharedString>>)> =
                db::run(cx, async move {
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
                })
                .await;

            let elapsed_ms = start.elapsed().as_millis() as u64;

            let _ = this.update(cx, |panel, cx| match result {
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
                    cx.update_global(|store: &mut QueryStore, _| {
                        store.push_history(HistoryEntry {
                            conn_id: conn_id.clone(),
                            query: sql_executed,
                            ran_at: OffsetDateTime::now_utc(),
                            duration_ms: elapsed_ms,
                            row_count: Some(row_count as u64),
                        });
                    });
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
        "SqliteQueryEditor"
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
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let err_fg = cx.theme().danger_foreground;

        let is_error = matches!(self.status, QueryStatus::Error(_));
        let err_text = match &self.status {
            QueryStatus::Error(s) => Some(s.clone()),
            _ => None,
        };

        let status_text: SharedString = match &self.status {
            QueryStatus::Idle => "Ready".into(),
            QueryStatus::Running => "Running…".into(),
            QueryStatus::Done { rows, elapsed_ms } => {
                format!("{rows} rows in {elapsed_ms}ms").into()
            }
            QueryStatus::Error(_) => "Query failed".into(),
        };

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(8.0))
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(
                Button::new("run")
                    .primary()
                    .label("Run")
                    .on_click(cx.listener(|panel, _, window, cx| panel.run_query(window, cx))),
            )
            .child(
                Button::new("sqlite-history-toggle")
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
                    .child(status_text),
            );

        let error_strip = err_text.map(|full| {
            let tip = full.clone();
            let line = notify::error_one_liner(&full);
            div()
                .id("sqlite-query-error-strip")
                .px(px(8.0))
                .pb(px(4.0))
                .text_xs()
                .text_color(err_fg)
                .truncate()
                .child(line)
                .tooltip(move |window, app| Tooltip::new(tip.clone()).build(window, app))
        });

        let bottom: AnyElement = if let Some(ref table) = self.result_table {
            div()
                .flex_1()
                .min_h(px(0.0))
                .child(read_only_striped(table))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("No results yet")
                .into_any_element()
        };

        let main_column = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(
                div()
                    .p(px(10.0))
                    .child(sql_editor::code_editor_area(
                        &self.sql_input,
                        is_error,
                        200.0,
                        cx,
                    )),
            )
            .child(bottom);

        let body =
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
                                            .id(("sqlite-hist", i))
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
                "Query Editor",
                "Run SQL, inspect result sets, recover history",
                cx,
            ))
            .child(toolbar)
            .when_some(error_strip, |col, strip| col.child(strip))
            .child(body)
    }
}
