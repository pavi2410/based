// postgres::query_editor — run ad-hoc SQL against a pool.

use gpui::{App, RenderOnce, prelude::*, *};
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

use crate::connection::{ConnectionId, EngineKind};
use crate::postgres::mutations::execute_sql;
use crate::project::ProjectRoot;
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::read_only_striped;
use crate::widgets::query_panel_extras::{
    self, HistoryFilter, filtered_history, save_starred_query,
};
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::tab_chip::tab_chip;
use crate::widgets::ui::{metadata_pill, panel_context_header};
use crate::widgets::virtual_table::{RowDelegate, replace_table_data};
use crate::workspace::pop_out::{PopOutManager, PopOutWindowTitle};
use crate::workspace::{
    TabSpec, enqueue_open_tab, mark_query_tab_dirty, notify, tab_open::take_sql_inject,
};

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
    show_variables: bool,
    history_filter: HistoryFilter,
    star_name: Option<String>,
    dirty: bool,
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
            show_variables: false,
            history_filter: HistoryFilter::default(),
            star_name: None,
            dirty: false,
        };
        let conn_for_dirty = panel.conn_id.clone();
        cx.observe(&sql_input, move |panel, _, cx| {
            if !panel.dirty {
                panel.dirty = true;
                mark_query_tab_dirty(&conn_for_dirty, cx);
            }
            cx.notify();
        })
        .detach();
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
                            replace_table_data(state, col_models, data, cx);
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

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        tab_chip(EngineKind::Postgres, "Query", self.dirty, false, cx)
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
            .child(
                Button::new("pg-vars-toggle")
                    .ghost()
                    .label(if self.show_variables {
                        "Hide vars"
                    } else {
                        "Variables"
                    })
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.show_variables = !panel.show_variables;
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
            .child(div().p_2().child(sql_editor::code_editor_area(
                &self.sql_input,
                is_error,
                200.0,
                cx,
            )))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(read_only_striped(&self.result)),
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
                row.child(HistorySidebarView {
                    panel: panel_ent.clone(),
                    filter: history_filter,
                    star_name: star_name.clone(),
                    entries,
                    border,
                    muted,
                })
            });

        let project_dir = cx.try_global::<ProjectRoot>().map(|p| p.0.clone());
        let show_variables = self.show_variables;
        let var_map = cx.global::<crate::project::ProjectVars>().vars.clone();
        let mono_font = cx.theme().mono_font_family.clone();
        let border_v = border;
        let muted_v = muted;
        let muted_bg = cx.theme().muted.opacity(0.06);

        v_flex()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(cx.theme().background)
            .when(
                !PopOutManager::is_pop_out_panel(cx.entity().entity_id(), cx),
                |col| {
                    col.child(panel_context_header(
                        "Run SQL, inspect plans, compare result sets",
                        cx,
                    ))
                },
            )
            .child(toolbar)
            .when_some(error_strip, |col, strip| col.child(strip))
            .child(editor_body)
            .child(query_panel_extras::variables_footer(
                project_dir,
                show_variables,
                var_map,
                mono_font,
                border_v,
                muted_v,
                muted_bg,
            ))
    }
}

#[derive(IntoElement)]
struct HistorySidebarView {
    panel: Entity<QueryEditorPanel>,
    filter: HistoryFilter,
    star_name: Option<String>,
    entries: Vec<HistoryEntry>,
    border: gpui::Hsla,
    muted: gpui::Hsla,
}

impl RenderOnce for HistorySidebarView {
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
                        Button::new(SharedString::from(format!("pg-hist-filter-{}", f.label())))
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
                                    Button::new("pg-star-save")
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
                                    Button::new("pg-star-cancel")
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
                    .id(SharedString::from(format!("pg-hist-{i}")))
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
                            .font_family("monospace")
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
                        Button::new(SharedString::from(format!("pg-star-{i}")))
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
