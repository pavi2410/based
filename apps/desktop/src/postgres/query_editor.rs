// postgres::query_editor — run ad-hoc SQL against a pool.

use std::rc::Rc;

use gpui::{App, prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{InputEvent, InputState},
    menu::PopupMenu,
    popover::Popover,
    resizable::{ResizableState, resizable_panel, v_resizable},
    scroll::ScrollableElement as _,
    spinner::Spinner,
    table::TableState,
    v_flex,
};
use sqlx::{AssertSqlSafe, PgPool, Row};

use crate::connection::ConnectionId;
use crate::postgres::execute_sql;
use crate::postgres::explain_plan::{PlanNode, parse_pg_explain_json, render_plan_node};
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::export;
use crate::widgets::query_panel_extras;
use crate::widgets::result_tabs::{BottomTab, result_tab_strip};
use crate::widgets::sql_editor::{self, new_sql_input, set_input_text, text_from_input};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::widgets::{metadata_pill, shortcut_run_kbd_in_primary_button};
use crate::workspace::pop_out::PopOutWindowTitle;
use crate::workspace::{mark_query_tab_dirty, tabs::take_sql_inject};

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

/// Result of an inline EXPLAIN (FORMAT JSON) run.
enum ExplainView {
    /// No explain has been requested yet — the tab shows an empty hint.
    Empty,
    /// Currently fetching the plan.
    Running,
    /// Parsed plan tree.
    Plan(PlanNode),
    /// Plan was returned as raw text (parsing failed or analyze disabled).
    Text(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    conn_id: ConnectionId,
    sql_input: Entity<InputState>,
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
    split_state: Entity<ResizableState>,
    bottom_tab: BottomTab,
    explain: ExplainView,
    dirty: bool,
    pub(crate) tab_label: SharedString,
    pub editor_ctx: gpui::Entity<crate::editor::EditorContext>,
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
        let result = cx.new(|cx| configure_row_table(delegate, window, cx));
        let sql_text = initial_sql.unwrap_or_else(|| "SELECT 1 AS one;".to_string());
        let sql_input = new_sql_input(&sql_text, window, cx);
        let split_state = cx.new(|_| ResizableState::default());
        let variables = cx
            .try_global::<crate::project::ProjectVars>()
            .map(|pv| crate::editor::VariableScope::from_string_map(&pv.vars))
            .unwrap_or_default();
        let editor_ctx = cx.new(|_| {
            crate::editor::EditorContext::new(
                conn_id.clone(),
                based_core::EngineKind::Postgres,
                variables,
            )
        });
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            conn_id,
            sql_input: sql_input.clone(),
            result,
            status: QueryStatus::Idle,
            split_state,
            bottom_tab: BottomTab::Results,
            explain: ExplainView::Empty,
            dirty: false,
            tab_label: "Query".into(),
            editor_ctx,
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

    pub(crate) fn connection_id(&self) -> &ConnectionId {
        &self.conn_id
    }

    pub fn load_sql(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        set_input_text(&self.sql_input, sql, window, cx);
        cx.notify();
    }

    pub fn current_sql(&self, cx: &App) -> String {
        text_from_input(&self.sql_input, cx)
    }

    /// Switch the bottom dock to the Explain tab and (re)run EXPLAIN inline.
    fn switch_to_explain(&mut self, cx: &mut Context<Self>) {
        let sql = self.current_sql(cx);
        self.bottom_tab = BottomTab::Explain;
        if sql.trim().is_empty() {
            self.explain = ExplainView::Empty;
            cx.notify();
            return;
        }
        self.explain = ExplainView::Running;
        cx.notify();
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            // `EXPLAIN (FORMAT JSON)` returns a `json` column (OID 114), not
            // `text`, so `try_get::<String, _>` would fail the sqlx type check.
            // `try_get_unchecked` skips that check and reads the wire bytes,
            // which for `json` in text format is the JSON literal as UTF-8.
            let outcome = crate::db::run(cx, async move {
                let q = format!("EXPLAIN (FORMAT JSON) {sql}");
                let rows = sqlx::query(AssertSqlSafe(q)).fetch_all(&pool).await?;
                let raw: String = match rows.first() {
                    Some(row) => row.try_get_unchecked::<String, _>(0)?,
                    None => return Ok::<_, anyhow::Error>(String::new()),
                };
                Ok(raw)
            })
            .await;
            let view = match outcome {
                Ok(raw) if raw.is_empty() => {
                    ExplainView::Text("(EXPLAIN returned no rows)".to_string())
                }
                Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                    Ok(json) => match parse_pg_explain_json(&json) {
                        Some(plan) => ExplainView::Plan(plan),
                        None => ExplainView::Text(raw),
                    },
                    Err(e) => ExplainView::Text(format!("(invalid JSON: {e})\n\n{raw}")),
                },
                Err(e) => ExplainView::Text(format!("EXPLAIN failed: {e}")),
            };
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.explain = view;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let sql_raw = self.current_sql(cx);
        if sql_raw.trim().is_empty() {
            return;
        }
        let project_vars = cx.global::<crate::project::ProjectVars>().vars.clone();
        let sql = crate::project::substitute(&sql_raw, &project_vars);
        let var_ctx = based_query::VariableContext {
            session: Default::default(),
            query: Default::default(),
            collection: Default::default(),
            environment: None,
            workspace: project_vars.clone(),
            connection: project_vars,
        };
        let sql = match based_query::resolve_query(&sql, &var_ctx) {
            Ok(resolved) => resolved,
            Err(e) => {
                self.status = QueryStatus::Error(e.to_string());
                self.bottom_tab = BottomTab::Messages;
                cx.notify();
                return;
            }
        };
        let sql_executed = sql.clone();
        let conn_id = self.conn_id.clone();
        self.status = QueryStatus::Running;
        self.bottom_tab = BottomTab::Results;
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
                            .map(|c| data_column(c.clone(), c))
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
                            store.push_history(HistoryEntry::new(
                                conn_id.clone(),
                                sql_executed,
                                ms,
                                Some(row_count as u64),
                                based_query::RunStatus::Ok,
                            ));
                        });
                        QueryStatus::Done {
                            rows: row_count,
                            affected: aff,
                            elapsed_ms: ms,
                        }
                    }
                    Err(e) => {
                        cx.update_global(|store: &mut QueryStore, _| {
                            store.push_history(HistoryEntry::new(
                                conn_id.clone(),
                                sql_executed,
                                ms,
                                None,
                                based_query::RunStatus::Error,
                            ));
                        });
                        panel.bottom_tab = BottomTab::Messages;
                        QueryStatus::Error(e.to_string())
                    }
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

    crate::based_panel_tab_chrome!(dirty);
}

impl PopOutWindowTitle for QueryEditorPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        "Query".into()
    }
}

/// Right-aligned status cluster shown at the end of the toolbar.
fn render_status_cluster(status: &QueryStatus, cx: &mut App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    match status {
        QueryStatus::Idle => h_flex()
            .gap(px(6.0))
            .items_center()
            .child(
                div()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(muted.opacity(0.55)),
            )
            .child(div().text_xs().text_color(muted).child("Ready"))
            .into_any_element(),
        QueryStatus::Running => h_flex()
            .gap(px(6.0))
            .items_center()
            .child(Spinner::new().xsmall().color(cx.theme().primary))
            .child(div().text_xs().text_color(muted).child("Running"))
            .into_any_element(),
        QueryStatus::Done {
            rows,
            affected,
            elapsed_ms,
        } => {
            let success = cx.theme().success_foreground;
            h_flex()
                .gap(px(6.0))
                .items_center()
                .child(
                    Icon::new(IconName::CircleCheck)
                        .text_color(success)
                        .xsmall(),
                )
                .child(metadata_pill("rows", rows.to_string(), cx))
                .child(metadata_pill("affected", affected.to_string(), cx))
                .child(metadata_pill("time", format!("{elapsed_ms} ms"), cx))
                .into_any_element()
        }
        QueryStatus::Error(_) => {
            let danger = cx.theme().danger_foreground;
            h_flex()
                .gap(px(6.0))
                .items_center()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .text_color(danger)
                        .xsmall(),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(danger)
                        .child("Failed"),
                )
                .into_any_element()
        }
    }
}

impl QueryEditorPanel {
    /// Render the body for the currently selected bottom tab.
    fn render_bottom_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.bottom_tab {
            BottomTab::Results => div()
                .flex_1()
                .min_h(px(0.0))
                .child(render_row_table(&self.result, cx))
                .into_any_element(),
            BottomTab::Messages => self.render_messages(cx),
            BottomTab::Explain => self.render_explain(cx),
        }
    }

    fn render_messages(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        match &self.status {
            QueryStatus::Error(full) => {
                let err_fg = theme.danger_foreground;
                let danger_bg = theme.danger.opacity(0.06);
                let danger_border = theme.danger.opacity(0.20);
                let mono = theme.mono_font_family.clone();
                let copy_text = full.clone();
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .p_3()
                    .child(
                        h_flex()
                            .id("pg-query-error-card")
                            .p_3()
                            .gap_2()
                            .items_start()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(danger_border)
                            .bg(danger_bg)
                            .child(
                                div().mt(px(2.0)).child(
                                    Icon::new(IconName::TriangleAlert)
                                        .text_color(err_fg)
                                        .xsmall(),
                                ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .font_family(mono)
                                    .text_color(err_fg)
                                    .child(full.clone()),
                            )
                            .child(
                                Button::new("pg-error-copy")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .tooltip(SharedString::from("Copy error"))
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy_text.clone(),
                                        ));
                                    }),
                            ),
                    )
                    .into_any_element()
            }
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
            } => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child(format!(
                    "Query OK · {rows} rows · {affected} affected · {elapsed_ms} ms"
                ))
                .into_any_element(),
            QueryStatus::Running => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child("Running…")
                .into_any_element(),
            QueryStatus::Idle => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child("No messages yet. Run a query to see output here.")
                .into_any_element(),
        }
    }

    fn render_explain(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        match &self.explain {
            ExplainView::Empty => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child("Click Explain in the toolbar to see the query plan.")
                .into_any_element(),
            ExplainView::Running => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child("Running EXPLAIN…")
                .into_any_element(),
            ExplainView::Plan(plan) => div()
                .id("pg-inline-explain")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .p(px(8.0))
                .child(render_plan_node(plan, 0, theme))
                .into_any_element(),
            ExplainView::Text(text) => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_sm()
                .font_family(mono)
                .text_color(theme.foreground)
                .child(text.clone())
                .into_any_element(),
        }
    }
}

impl Render for QueryEditorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(sql) = take_sql_inject(&self.conn_id, cx) {
            set_input_text(&self.sql_input, &sql, window, cx);
        }
        let border = cx.theme().border;

        let is_error = matches!(self.status, QueryStatus::Error(_));

        let project_dir = cx
            .try_global::<crate::project::ProjectRoot>()
            .map(|p| p.0.clone());
        let var_map = cx.global::<crate::project::ProjectVars>().vars.clone();
        let mono_font = cx.theme().mono_font_family.clone();

        let (export_headers, export_rows) = {
            let st = self.result.read(cx);
            let d = st.delegate();
            let h = d
                .columns
                .iter()
                .map(|c| c.key.to_string())
                .collect::<Vec<_>>();
            let r = d
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect::<Vec<Vec<String>>>();
            (h, r)
        };
        let export_popover = {
            let (h, r) = (export_headers.clone(), export_rows.clone());
            let (h2, r2) = (export_headers.clone(), export_rows.clone());
            Popover::new("pg-export-popover")
                .trigger(
                    Button::new("pg-export-trigger")
                        .ghost()
                        .small()
                        .label("Export"),
                )
                .content(move |_, _, _| {
                    let (hc, rc) = (h.clone(), r.clone());
                    let (hx, rx) = (h2.clone(), r2.clone());
                    v_flex()
                        .gap(px(2.0))
                        .p(px(4.0))
                        .child(
                            Button::new("pg-export-csv")
                                .ghost()
                                .small()
                                .label("CSV")
                                .on_click(move |_, _, cx| {
                                    let (hc, rc) = (hc.clone(), rc.clone());
                                    cx.spawn(async move |cx| {
                                        if let Ok(bytes) = export::to_csv(&hc, &rc)
                                            && let Some(path) = export::save_bytes(
                                                cx,
                                                "export.csv",
                                                "CSV",
                                                &["csv"],
                                                bytes,
                                            )
                                            .await
                                        {
                                            cx.update(|app| {
                                                crate::workspace::notify::push_export_success(
                                                    app, &path,
                                                )
                                            });
                                        }
                                    })
                                    .detach();
                                }),
                        )
                        .child(
                            Button::new("pg-export-xlsx")
                                .ghost()
                                .small()
                                .label("Excel (.xlsx)")
                                .on_click(move |_, _, cx| {
                                    let (hx, rx) = (hx.clone(), rx.clone());
                                    cx.spawn(async move |cx| {
                                        if let Ok(bytes) = export::to_xlsx(&hx, &rx)
                                            && let Some(path) = export::save_bytes(
                                                cx,
                                                "export.xlsx",
                                                "Excel",
                                                &["xlsx"],
                                                bytes,
                                            )
                                            .await
                                        {
                                            cx.update(|app| {
                                                crate::workspace::notify::push_export_success(
                                                    app, &path,
                                                )
                                            });
                                        }
                                    })
                                    .detach();
                                }),
                        )
                })
        };

        let toolbar = h_flex()
            .gap(px(6.0))
            .px_2()
            .py(px(4.0))
            .items_center()
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(
                Button::new("pg-run")
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label("Run")
                    .child(shortcut_run_kbd_in_primary_button(cx))
                    .on_click(cx.listener(|panel, _, _, cx| panel.run(cx))),
            )
            .child(
                Button::new("pg-explain")
                    .ghost()
                    .small()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.switch_to_explain(cx))),
            )
            .child(query_panel_extras::variables_popover(
                "pg-vars-popover",
                project_dir,
                var_map,
                mono_font,
                cx,
            ))
            .child(export_popover)
            .child(div().flex_1())
            .child(render_status_cluster(&self.status, cx));

        let editor_pane = div().size_full().p_2().child(sql_editor::code_editor_flex(
            &self.sql_input,
            is_error,
            cx,
        ));

        let on_select: Rc<dyn Fn(BottomTab, &mut Window, &mut App)> = {
            let entity = cx.entity();
            Rc::new(move |tab, _, cx| {
                entity.update(cx, |panel, cx| {
                    panel.bottom_tab = tab;
                    cx.notify();
                });
            })
        };
        let has_error = matches!(self.status, QueryStatus::Error(_));
        let strip = result_tab_strip("pg-bt", self.bottom_tab, has_error, on_select, cx);
        let bottom_body = self.render_bottom_body(cx);
        let bottom_pane = v_flex().size_full().child(strip).child(bottom_body);

        v_flex()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(cx.theme().background)
            .child(toolbar)
            .child(
                div().flex_1().min_h(px(0.0)).child(
                    v_resizable("pg-query-split")
                        .with_state(&self.split_state)
                        .child(resizable_panel().size(px(220.0)).child(editor_pane))
                        .child(resizable_panel().child(bottom_pane)),
                ),
            )
    }
}
