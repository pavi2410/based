// postgres::query_editor — run ad-hoc SQL against a pool.

use gpui::{App, prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{InputEvent, InputState},
    menu::PopupMenu,
    spinner::Spinner,
    table::TableState,
    v_flex,
};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::connection::ConnectionId;
use crate::postgres::mutations::execute_sql;
use crate::project::ProjectRoot;
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::query_panel_extras;
use crate::widgets::sql_editor::{self, new_sql_input, set_sql_input, sql_from_input};
use crate::widgets::ui::{metadata_pill, shortcut_run_kbd_in_primary_button};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::workspace::pop_out::PopOutWindowTitle;
use crate::workspace::{
    TabSpec, enqueue_open_tab, mark_query_tab_dirty, tab_open::take_sql_inject,
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
    show_variables: bool,
    dirty: bool,
    pub(crate) tab_label: SharedString,
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
        let panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            conn_id,
            sql_input: sql_input.clone(),
            result,
            status: QueryStatus::Idle,
            show_variables: false,
            dirty: false,
            tab_label: "Query".into(),
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

    fn tab_name(&self, _: &gpui::App) -> Option<gpui::SharedString> {
        Some(crate::workspace::tab_label::with_dirty_suffix(
            &self.tab_label,
            self.dirty,
        ))
    }

    fn zoomable(&self, _: &gpui::App) -> Option<gpui_component::dock::PanelControl> {
        None
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        crate::workspace::tab_label::with_dirty_suffix(&self.tab_label, self.dirty)
    }
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

        let show_variables = self.show_variables;

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
                    .on_click(cx.listener(|panel, _, _, cx| panel.open_explain(cx))),
            )
            .child(
                Button::new("pg-vars-toggle")
                    .ghost()
                    .small()
                    .selected(show_variables)
                    .label("Variables")
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.show_variables = !panel.show_variables;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(render_status_cluster(&self.status, cx));

        let mono_font_err = cx.theme().mono_font_family.clone();
        let danger_bg = cx.theme().danger.opacity(0.06);
        let danger_border = cx.theme().danger.opacity(0.20);
        let error_strip = err_text.map(|full| {
            let copy_text = full.clone();
            h_flex()
                .id("pg-query-error-card")
                .mx_2()
                .my_1()
                .px_3()
                .py_2()
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
                        .max_h(px(72.0))
                        .overflow_hidden()
                        .text_xs()
                        .font_family(mono_font_err.clone())
                        .text_color(err_fg)
                        .child(full),
                )
                .child(
                    Button::new("pg-error-copy")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Copy)
                        .tooltip(SharedString::from("Copy error"))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
                        }),
                )
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
                        200.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(render_row_table(&self.result, cx)),
            );

        let editor_body = h_flex().flex_1().min_h(px(0.0)).child(main_column);

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
