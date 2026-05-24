// sqlite::query_editor — QueryEditorPanel: run arbitrary SQL and view results.

use gpui::{App, prelude::*, *};
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
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::query_panel_extras::{HistoryFilter, filtered_history, save_starred_query};
use crate::widgets::row_cell::sqlite_cell_display;
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::ui::{panel_context_header, shortcut_run_kbd_in_primary_button};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::workspace::pop_out::{PopOutManager, PopOutWindowTitle};
use crate::workspace::{TabSpec, enqueue_open_tab, notify, tab_open::take_sql_inject};

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
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
    show_history: bool,
    history_filter: HistoryFilter,
    star_name: Option<String>,
    pub(crate) tab_label: SharedString,
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
        let delegate = RowDelegate::default();
        let result = cx.new(|cx| configure_row_table(delegate, window, cx));
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            conn_id,
            sql_input: sql_input.clone(),
            result,
            status: QueryStatus::Idle,
            show_history: false,
            history_filter: HistoryFilter::default(),
            star_name: None,
            tab_label: "Query".into(),
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

    fn run_query(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql_raw = self.current_sql(cx);
        let vars = cx.global::<crate::project::ProjectVars>().vars.clone();
        let sql = crate::project::substitute(&sql_raw, &vars);
        let sql_executed = sql.clone();
        let conn_id = self.conn_id.clone();
        self.status = QueryStatus::Running;

        cx.spawn(async move |this, cx| {
            let start = std::time::Instant::now();

            let result: anyhow::Result<(Vec<Column>, Vec<Vec<SharedString>>)> =
                db::run(cx, async move {
                    let rows = sqlx::query(&sql).fetch_all(&pool).await?;
                    let columns: Vec<Column> = if let Some(first) = rows.first() {
                        first
                            .columns()
                            .iter()
                            .map(|c| data_column(c.name().to_string(), c.name().to_string()))
                            .collect()
                    } else {
                        vec![]
                    };
                    let data_rows: Vec<Vec<SharedString>> = rows
                        .iter()
                        .map(|row| {
                            (0..row.len())
                                .map(|i| SharedString::from(sqlite_cell_display(row, i)))
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
                    panel.result.update(cx, |state, cx| {
                        replace_table_data(state, columns, data_rows, cx);
                    });
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

    crate::based_panel_tab_chrome!();

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_label.clone()
    }
}

impl PopOutWindowTitle for QueryEditorPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        "Query".into()
    }
}

impl Render for QueryEditorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(sql) = take_sql_inject(&self.conn_id, cx) {
            set_sql_input(&self.sql_input, &sql, window, cx);
        }
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
                    .child(shortcut_run_kbd_in_primary_button(cx))
                    .on_click(cx.listener(|panel, _, window, cx| panel.run_query(window, cx))),
            )
            .child(
                Button::new("sqlite-explain")
                    .ghost()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.open_explain(cx))),
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

        let main_column = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .child(
                div()
                    .flex_shrink_0()
                    .p_2()
                    .child(sql_editor::code_editor_area(
                        &self.sql_input,
                        is_error,
                        120.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(render_row_table(&self.result, cx)),
            );

        let panel_ent = cx.entity();
        let history_entries = if self.show_history {
            let store = cx.global::<QueryStore>();
            Some(filtered_history(store, &self.conn_id, self.history_filter))
        } else {
            None
        };
        let history_filter = self.history_filter;
        let star_name = self.star_name.clone();

        let editor_body = h_flex()
            .flex_1()
            .min_h(px(0.0))
            .child(main_column)
            .when_some(history_entries, |row, entries| {
                row.child(SqliteHistorySidebar {
                    panel: panel_ent.clone(),
                    filter: history_filter,
                    star_name: star_name.clone(),
                    entries,
                    border,
                    muted,
                })
            });

        v_flex()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(cx.theme().background)
            .when(
                !PopOutManager::is_pop_out_panel(cx.entity().entity_id(), cx),
                |col| {
                    col.child(panel_context_header(
                        "Run SQL, inspect result sets, recover history",
                        cx,
                    ))
                },
            )
            .child(toolbar)
            .when_some(error_strip, |col, strip| col.child(strip))
            .child(editor_body)
    }
}

#[derive(IntoElement)]
struct SqliteHistorySidebar {
    panel: Entity<QueryEditorPanel>,
    filter: HistoryFilter,
    star_name: Option<String>,
    entries: Vec<HistoryEntry>,
    border: gpui::Hsla,
    muted: gpui::Hsla,
}

impl gpui::RenderOnce for SqliteHistorySidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui::BorrowAppContext as _;
        use gpui_component::Sizable as _;

        let panel_ent = self.panel;
        let filter = self.filter;
        let border = self.border;
        let muted = self.muted;
        let star_name = self.star_name;
        let entries = self.entries;

        v_flex()
            .w(px(260.0))
            .min_h(px(0.0))
            .border_l_1()
            .border_color(border)
            .bg(cx.theme().muted.opacity(0.08))
            .child(
                v_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("History"),
                    )
                    .child(h_flex().gap_1().children(HistoryFilter::ALL.map(|f| {
                        let active = filter == f;
                        let panel_ent = panel_ent.clone();
                        Button::new(SharedString::from(format!(
                            "sqlite-hist-filter-{}",
                            f.label()
                        )))
                        .ghost()
                        .xsmall()
                        .label(f.label())
                        .when(active, |b| b.primary())
                        .on_click(move |_, _, cx| {
                            panel_ent.update(cx, |panel, cx| {
                                panel.history_filter = f;
                                cx.notify();
                            });
                        })
                    })))
                    .when_some(star_name, |col, name| {
                        let panel_save = panel_ent.clone();
                        let panel_cancel = panel_ent.clone();
                        let name = name.clone();
                        col.child(
                            h_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("Save as \"{name}\"")),
                                )
                                .child(
                                    Button::new("sqlite-star-save")
                                        .primary()
                                        .xsmall()
                                        .label("Save")
                                        .on_click(move |_, _, cx| {
                                            panel_save.update(cx, |panel, cx| {
                                                let sql = panel.current_sql(cx);
                                                let conn = panel.conn_id.clone();
                                                cx.update_global(|store: &mut QueryStore, _| {
                                                    save_starred_query(
                                                        store, conn, &name, &sql, false, None,
                                                    );
                                                });
                                                panel.star_name = None;
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("sqlite-star-cancel")
                                        .ghost()
                                        .xsmall()
                                        .label("Cancel")
                                        .on_click(move |_, _, cx| {
                                            panel_cancel.update(cx, |panel, cx| {
                                                panel.star_name = None;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                    }),
            )
            .children(entries.into_iter().enumerate().map(|(i, e)| {
                let preview: SharedString = e.query.chars().take(80).collect::<String>().into();
                let full_query = e.query.clone();
                let panel_click = panel_ent.clone();
                let panel_star = panel_ent.clone();
                let sql_input = panel_click.read(cx).sql_input.clone();
                h_flex()
                    .id(SharedString::from(format!("sqlite-hist-{i}")))
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
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(muted)
                            .child(preview)
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                panel_click.update(cx, |_panel, cx| {
                                    set_sql_input(&sql_input, &full_query, window, cx);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("sqlite-star-{i}")))
                            .ghost()
                            .xsmall()
                            .label("★")
                            .on_click(move |_, _, cx| {
                                panel_star.update(cx, |panel, cx| {
                                    panel.star_name = Some(format!("query_{i}"));
                                    cx.notify();
                                });
                            }),
                    )
            }))
    }
}
