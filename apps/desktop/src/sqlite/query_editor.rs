// sqlite::query_editor — QueryEditorPanel: run arbitrary SQL and view results.

use std::rc::Rc;

use gpui::{App, prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{InputEvent, InputState},
    menu::PopupMenu,
    resizable::{ResizableState, resizable_panel, v_resizable},
    table::{Column, TableState},
    v_flex,
};
use sqlx::{Column as SqlxColumn, Row, SqlitePool};
use time::OffsetDateTime;

use super::eqp_parse::{EqpNode, parse_eqp};
use super::eqp_viewer::render_eqp_body;

use crate::connection::ConnectionId;
use crate::db;
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::query_panel_extras;
use crate::widgets::result_tabs::{BottomTab, result_tab_strip};
use crate::widgets::row_cell::sqlite_cell_display;
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::ui::{panel_context_header, shortcut_run_kbd_in_primary_button};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::workspace::pop_out::{PopOutManager, PopOutWindowTitle};
use crate::workspace::tab_open::take_sql_inject;

pub enum QueryStatus {
    Idle,
    Running,
    Done { rows: usize, elapsed_ms: u64 },
    Error(String),
}

/// Result of an inline EXPLAIN QUERY PLAN run.
enum ExplainView {
    Empty,
    Running,
    Plan(Vec<EqpNode>),
    Text(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    conn_id: ConnectionId,
    sql_input: Entity<InputState>,
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
    split_state: Entity<ResizableState>,
    bottom_tab: BottomTab,
    explain: ExplainView,
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
        let split_state = cx.new(|_| ResizableState::default());
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

    /// Switch the bottom dock to the Explain tab and (re)run EXPLAIN QUERY PLAN inline.
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
            let outcome = db::run(cx, async move {
                let q = format!("EXPLAIN QUERY PLAN {sql}");
                Ok(sqlx::query(&q).fetch_all(&pool).await?)
            })
            .await;
            let view = match outcome {
                Ok(rows) => {
                    let flat: Vec<(i64, i64, String)> = rows
                        .iter()
                        .map(|row| {
                            let id: i64 = row.try_get("id").unwrap_or(0);
                            let parent: i64 = row.try_get("parent").unwrap_or(0);
                            let detail: String = row.try_get("detail").unwrap_or_default();
                            (id, parent, detail)
                        })
                        .collect();
                    let roots = parse_eqp(&flat);
                    if roots.is_empty() {
                        ExplainView::Text("(EXPLAIN QUERY PLAN returned no rows)".to_string())
                    } else {
                        ExplainView::Plan(roots)
                    }
                }
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

    fn run_query(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql_raw = self.current_sql(cx);
        let vars = cx.global::<crate::project::ProjectVars>().vars.clone();
        let sql = crate::project::substitute(&sql_raw, &vars);
        let sql_executed = sql.clone();
        let conn_id = self.conn_id.clone();
        self.status = QueryStatus::Running;
        self.bottom_tab = BottomTab::Results;

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
                    panel.bottom_tab = BottomTab::Messages;
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

impl QueryEditorPanel {
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
                            .id("sqlite-query-error-card")
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
                                Button::new("sqlite-error-copy")
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
            QueryStatus::Done { rows, elapsed_ms } => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child(format!("Query OK · {rows} rows · {elapsed_ms} ms"))
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
                .child("Running EXPLAIN QUERY PLAN…")
                .into_any_element(),
            ExplainView::Plan(roots) => div()
                .flex_1()
                .min_h(px(0.0))
                .child(render_eqp_body("sqlite-inline-eqp", roots, theme))
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
            set_sql_input(&self.sql_input, &sql, window, cx);
        }
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let err_fg = cx.theme().danger_foreground;

        let is_error = matches!(self.status, QueryStatus::Error(_));

        let status_text: SharedString = match &self.status {
            QueryStatus::Idle => "Ready".into(),
            QueryStatus::Running => "Running…".into(),
            QueryStatus::Done { rows, elapsed_ms } => {
                format!("{rows} rows in {elapsed_ms}ms").into()
            }
            QueryStatus::Error(_) => "Query failed".into(),
        };

        let project_dir = cx
            .try_global::<crate::project::ProjectRoot>()
            .map(|p| p.0.clone());
        let var_map = cx.global::<crate::project::ProjectVars>().vars.clone();
        let mono_font = cx.theme().mono_font_family.clone();

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
                    .on_click(cx.listener(|panel, _, _, cx| panel.switch_to_explain(cx))),
            )
            .child(query_panel_extras::variables_popover(
                "sqlite-vars-popover",
                project_dir,
                var_map,
                mono_font,
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(if is_error { err_fg } else { muted })
                    .child(status_text),
            );

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
        let strip = result_tab_strip("sqlite-bt", self.bottom_tab, has_error, on_select, cx);
        let bottom_body = self.render_bottom_body(cx);
        let bottom_pane = v_flex().size_full().child(strip).child(bottom_body);

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
            .child(
                div().flex_1().min_h(px(0.0)).child(
                    v_resizable("sqlite-query-split")
                        .with_state(&self.split_state)
                        .child(resizable_panel().size(px(180.0)).child(editor_pane))
                        .child(resizable_panel().child(bottom_pane)),
                ),
            )
    }
}
