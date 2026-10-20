// SQL query editor shared by SQL engines via [`QuerySource`]: editor, results, messages, explain.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use based_core::EngineKind;
use futures::future::BoxFuture;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, Theme,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{EditorState, InputEvent},
    menu::PopupMenu,
    resizable::{ResizableState, resizable_panel, v_resizable},
    table::TableState,
    v_flex,
};
use gpui_kit::{App, Entity, prelude::*, *};

use crate::app::prefs;
use crate::connection::{ConnectionId, is_connection_read_only};
use crate::db;
use crate::editor::SchemaCache;
use crate::project::{ProjectRoot, ProjectVars, substitute};
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::export_popover::export_popover;
use crate::widgets::metadata_pill;
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::query_panel_extras;
use crate::widgets::query_status::{
    QueryStatusDisplay, query_error_card, query_status_indicator, tab_breadcrumb_query_trailing,
};
use crate::widgets::result_tabs::{BottomTab, result_tab_strip};
use crate::widgets::shortcut_run_kbd_in_primary_button;
use crate::widgets::sql_editor::{self, new_sql_input, set_input_text, text_from_input};
use crate::widgets::virtual_table::{
    RowDelegate, data_column, meta_from_query_type, replace_table_data,
};
use crate::workspace::pop_out::PopOutWindowTitle;
use crate::workspace::{WorkspaceRef, mark_query_tab_dirty, tabs::take_sql_inject};

pub type Fut<T> = BoxFuture<'static, anyhow::Result<T>>;

/// Result of running a script: last result set (name, type) + cells, and rows affected if known.
#[derive(Default)]
pub struct RunOutput {
    pub columns: Vec<(String, String)>,
    pub rows: Vec<Vec<String>>,
    pub affected: Option<u64>,
}

/// Engine-specific query plan, rendered inline.
pub trait PlanView: Send {
    fn render(&self, theme: &Theme) -> AnyElement;
}

pub enum Explained {
    Plan(Box<dyn PlanView>),
    Text(String),
}

/// Engine-specific execution for [`QueryEditorPanel`].
pub trait QuerySource: Send + Sync + 'static {
    /// Dock panel name, e.g. `"PgQueryEditor"`.
    fn panel_name(&self) -> &'static str;
    /// Element id prefix, e.g. `"pg"`.
    fn id(&self) -> &'static str;
    fn engine(&self) -> EngineKind;
    fn default_sql(&self) -> &'static str;
    /// Schema for completions; `None` disables completion.
    fn load_schema(&self) -> Option<Fut<SchemaCache>> {
        None
    }
    fn run(&self, sql: String, timeout_secs: u32) -> Fut<RunOutput>;
    fn explain(&self, sql: String) -> Fut<Explained>;
}

pub enum QueryStatus {
    Idle,
    Running,
    Done {
        rows: usize,
        affected: Option<u64>,
        elapsed_ms: u64,
    },
    Error(String),
}

enum ExplainView {
    Empty,
    Running,
    Plan(Box<dyn PlanView>),
    Text(String),
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    source: Arc<dyn QuerySource>,
    conn_id: ConnectionId,
    sql_input: Entity<EditorState>,
    result: Entity<TableState<RowDelegate>>,
    status: QueryStatus,
    split_state: Entity<ResizableState>,
    bottom_tab: BottomTab,
    explain: ExplainView,
    dirty: bool,
    pub(crate) tab_label: SharedString,
}

impl QueryEditorPanel {
    pub fn new(
        source: Arc<dyn QuerySource>,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_initial(source, conn_id, None, true, window, cx)
    }

    pub fn new_with_initial(
        source: Arc<dyn QuerySource>,
        conn_id: ConnectionId,
        initial_sql: Option<String>,
        auto_run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sql = initial_sql.unwrap_or_else(|| source.default_sql().to_string());
        let sql_input = new_sql_input(&sql, window, cx);
        let result = cx.new(|cx| configure_row_table(RowDelegate::default(), window, cx));
        let split_state = cx.new(|_| ResizableState::default());
        let panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            sql_input: sql_input.clone(),
            result,
            status: QueryStatus::Idle,
            split_state,
            bottom_tab: BottomTab::Results,
            explain: ExplainView::Empty,
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
        if auto_run && !sql.trim().is_empty() {
            cx.defer_in(window, |panel, _, cx| panel.run(cx));
        }
        panel.load_schema(cx);
        panel
    }

    fn load_schema(&self, cx: &mut Context<Self>) {
        let Some(load) = self.source.load_schema() else {
            return;
        };
        let sql_input = self.sql_input.clone();
        let empty = Arc::new(SchemaCache::new(self.source.engine()));
        sql_editor::attach_sql_completion(&sql_input, empty, cx);
        cx.spawn(async move |_, cx| {
            let Ok(cache) = db::run(cx, load).await else {
                return;
            };
            cx.update(|cx| sql_editor::attach_sql_completion(&sql_input, Arc::new(cache), cx));
        })
        .detach();
    }

    pub(crate) fn connection_id(&self) -> &ConnectionId {
        &self.conn_id
    }

    pub fn current_sql(&self, cx: &App) -> String {
        text_from_input(&self.sql_input, cx)
    }

    /// Switch to the Explain tab and (re)run EXPLAIN inline.
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
        let fut = self.source.explain(sql);
        cx.spawn(async move |this, cx| {
            let view = match db::run(cx, fut).await {
                Ok(Explained::Plan(p)) => ExplainView::Plan(p),
                Ok(Explained::Text(t)) => ExplainView::Text(t),
                Err(e) => ExplainView::Text(format!("EXPLAIN failed: {e}")),
            };
            let _ = this.update(cx, |panel, cx| {
                panel.explain = view;
                cx.notify();
            });
        })
        .detach();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let sql_raw = self.current_sql(cx);
        if sql_raw.trim().is_empty() {
            return;
        }
        let project_vars = cx.global::<ProjectVars>().vars.clone();
        let var_ctx = based_query::VariableContext {
            session: Default::default(),
            query: Default::default(),
            collection: Default::default(),
            environment: None,
            workspace: project_vars.clone(),
            connection: project_vars.clone(),
        };
        let sql = match based_query::resolve_query(&substitute(&sql_raw, &project_vars), &var_ctx) {
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
        let fut = self.source.run(sql, prefs::query_timeout_secs(cx));
        cx.spawn(async move |this, cx| {
            let start = Instant::now();
            let outcome = db::run(cx, fut).await;
            let ms = start.elapsed().as_millis() as u64;
            let _ = this.update(cx, |panel, cx| {
                let (rows, status) = match outcome {
                    Ok(out) => {
                        let columns = out
                            .columns
                            .iter()
                            .map(|(n, _)| data_column(n.clone(), n.clone()))
                            .collect();
                        let meta = out
                            .columns
                            .iter()
                            .map(|(_, t)| meta_from_query_type(t))
                            .collect();
                        let data: Vec<Vec<SharedString>> = out
                            .rows
                            .into_iter()
                            .map(|r| r.into_iter().map(SharedString::from).collect())
                            .collect();
                        let n = data.len();
                        panel.result.update(cx, |state, cx| {
                            replace_table_data(state, columns, data, meta, cx);
                        });
                        let status = QueryStatus::Done {
                            rows: n,
                            affected: out.affected,
                            elapsed_ms: ms,
                        };
                        (Some(n as u64), status)
                    }
                    Err(e) => {
                        panel.bottom_tab = BottomTab::Messages;
                        (None, QueryStatus::Error(e.to_string()))
                    }
                };
                let run_status = if rows.is_some() {
                    based_query::RunStatus::Ok
                } else {
                    based_query::RunStatus::Error
                };
                cx.update_global(|store: &mut QueryStore, _| {
                    store.push_history(HistoryEntry::new(
                        conn_id,
                        sql_executed,
                        ms,
                        rows,
                        run_status,
                    ));
                });
                panel.status = status;
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

impl BasePanel for QueryEditorPanel {
    fn panel_name(&self) -> &'static str {
        self.source.panel_name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for QueryEditorPanel {
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

fn status_display(status: &QueryStatus) -> QueryStatusDisplay {
    match status {
        QueryStatus::Idle => QueryStatusDisplay::Idle,
        QueryStatus::Running => QueryStatusDisplay::Running,
        QueryStatus::Done {
            rows,
            affected,
            elapsed_ms,
        } => QueryStatusDisplay::Done {
            rows: *rows,
            affected: *affected,
            elapsed_ms: *elapsed_ms,
        },
        QueryStatus::Error(e) => QueryStatusDisplay::Error(e.clone().into()),
    }
}

fn hint(text: impl Into<SharedString>, muted: Hsla) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .p_3()
        .text_xs()
        .text_color(muted)
        .child(text.into())
        .into_any_element()
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
        let muted = cx.theme().muted_foreground;
        match &self.status {
            QueryStatus::Error(full) => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .child(query_error_card(
                    format!("{}-query-error-card", self.source.id()),
                    full.clone().into(),
                    cx,
                ))
                .into_any_element(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
            } => {
                let affected = affected.map_or(String::new(), |a| format!(" · {a} affected"));
                hint(
                    format!("Query OK · {rows} rows{affected} · {elapsed_ms} ms"),
                    muted,
                )
            }
            QueryStatus::Running => hint("Running…", muted),
            QueryStatus::Idle => hint("No messages yet. Run a query to see output here.", muted),
        }
    }

    fn render_explain(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        match &self.explain {
            ExplainView::Empty => {
                hint("Click Explain in the toolbar to see the query plan.", muted)
            }
            ExplainView::Running => hint("Running EXPLAIN…", muted),
            ExplainView::Plan(plan) => div()
                .flex_1()
                .min_h(px(0.0))
                .child(plan.render(theme))
                .into_any_element(),
            ExplainView::Text(text) => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_sm()
                .font_family(theme.mono_font_family.clone())
                .text_color(theme.foreground)
                .child(text.clone())
                .into_any_element(),
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = self.source.id();
        let (headers, rows) = {
            let d = self.result.read(cx).delegate();
            (
                d.columns.iter().map(|c| c.key.to_string()).collect(),
                d.rows
                    .iter()
                    .map(|row| row.iter().map(|c| c.to_string()).collect())
                    .collect(),
            )
        };
        let read_only = cx
            .try_global::<WorkspaceRef>()
            .map(|ws| is_connection_read_only(&self.conn_id, ws.0.read(cx).registry().read(cx), cx))
            .unwrap_or(false);
        let muted = cx.theme().muted_foreground;
        let project_dir = cx.try_global::<ProjectRoot>().map(|p| p.0.clone());
        let var_map = cx.global::<ProjectVars>().vars.clone();
        let mono_font = cx.theme().mono_font_family.clone();

        h_flex()
            .gap(px(6.0))
            .px_2()
            .py(px(4.0))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(
                Button::new(format!("{id}-run"))
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label("Run")
                    .child(shortcut_run_kbd_in_primary_button(cx))
                    .on_click(cx.listener(|panel, _, _, cx| panel.run(cx))),
            )
            .child(
                Button::new(format!("{id}-explain"))
                    .ghost()
                    .small()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.switch_to_explain(cx))),
            )
            .when(read_only, |h| {
                h.child(metadata_pill("access", "Read-only", cx)).child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("Writes will fail on this connection"),
                )
            })
            .child(query_panel_extras::variables_popover(
                format!("{id}-vars-popover"),
                project_dir,
                var_map,
                mono_font,
                cx,
            ))
            .child(export_popover(&format!("{id}-qe"), headers, rows))
            .child(div().flex_1())
            .child(query_status_indicator(&status_display(&self.status), cx))
    }
}

impl Render for QueryEditorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(sql) = take_sql_inject(&self.conn_id, cx) {
            set_input_text(&self.sql_input, &sql, window, cx);
        }
        let id = self.source.id();
        let has_error = matches!(self.status, QueryStatus::Error(_));
        let toolbar = self.render_toolbar(cx);
        let editor_pane = div().size_full().p_2().child(sql_editor::code_editor_flex(
            &self.sql_input,
            has_error,
            false,
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
        let strip = result_tab_strip(
            &format!("{id}-bt"),
            self.bottom_tab,
            has_error,
            on_select,
            cx,
        );
        let bottom_pane = v_flex()
            .size_full()
            .child(strip)
            .child(self.render_bottom_body(cx));

        let display = status_display(&self.status);
        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, ["Query"], cx);
        let footer = tab_breadcrumb_footer(
            format!("{id}-qe-breadcrumb"),
            crumbs,
            tab_breadcrumb_query_trailing(&display, cx),
            cx,
        );

        panel_tab_content(
            v_flex()
                .w_full()
                .h_full()
                .min_h(px(0.0))
                .child(toolbar)
                .child(
                    div().flex_1().min_h(px(0.0)).child(
                        v_resizable(SharedString::from(format!("{id}-query-split")))
                            .with_state(&self.split_state)
                            .child(resizable_panel().size(px(220.0)).child(editor_pane))
                            .child(resizable_panel().child(bottom_pane)),
                    ),
                ),
            footer,
        )
    }
}
