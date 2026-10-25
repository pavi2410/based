// SQL query editor shared by SQL engines via [`QuerySource`]: editor, results, messages, explain.

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

use based_core::EngineKind;
use based_query::{Lint, Severity};
use futures::future::BoxFuture;
use gpui_kit::component::{
    ActiveTheme, IconName, Selectable as _, Sizable as _, Theme,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{EditorState, InputEvent, RopeExt as _},
    menu::PopupMenu,
    resizable::{ResizableState, resizable_panel, v_resizable},
    table::TableState,
    v_flex,
};
use gpui_kit::{App, Entity, prelude::*, *};

use crate::ai::AiHost;
use crate::app::prefs;
use crate::bindings::AskAi;
use crate::connection::{ConnectionId, is_connection_read_only};
use crate::db;
use crate::editor::sql_completion::InlineAi;
use crate::editor::{SchemaCache, lint::lint};
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
use crate::workspace::{WorkspaceRef, tabs::mark_tab_dirty};

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
        finished_at: OffsetDateTime,
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
    pub(super) source: Arc<dyn QuerySource>,
    pub(super) conn_id: ConnectionId,
    pub(super) sql_input: Entity<EditorState>,
    pub(super) schema: Arc<SchemaCache>,
    lint_seq: u64,
    pub(super) result: Entity<TableState<RowDelegate>>,
    pub(super) status: QueryStatus,
    split_state: Entity<ResizableState>,
    bottom_tab: BottomTab,
    explain: ExplainView,
    dirty: bool,
    pub(super) ai: AiHost,
    /// A write from an AI proposal is waiting on the confirmation banner.
    pub(super) ai_confirm: bool,
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
        let source_engine = source.engine();
        let sql = initial_sql.unwrap_or_else(|| source.default_sql().to_string());
        let sql_input = new_sql_input(&sql, window, cx);
        let result = cx.new(|cx| configure_row_table(RowDelegate::default(), window, cx));
        let split_state = cx.new(|_| ResizableState::default());
        let panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            sql_input: sql_input.clone(),
            schema: Arc::new(SchemaCache::new(source_engine)),
            lint_seq: 0,
            result,
            status: QueryStatus::Idle,
            split_state,
            bottom_tab: BottomTab::Results,
            explain: ExplainView::Empty,
            dirty: false,
            ai: AiHost::new(window, cx, Self::on_ai_event),
            ai_confirm: false,
            tab_label: "Query".into(),
        };
        cx.subscribe_in(&sql_input, window, |panel, _, event, _, cx| match event {
            InputEvent::PressEnter {
                secondary: true,
                shift: false,
            } => panel.run(cx),
            InputEvent::Change => {
                // Edits only: focus and selection changes also notify the editor.
                panel.dirty = true;
                mark_tab_dirty(cx.entity_id(), cx);
                panel.schedule_lint(cx);
            }
            _ => {}
        })
        .detach();
        if auto_run && !sql.trim().is_empty() {
            cx.defer_in(window, |panel, _, cx| panel.run(cx));
        }
        panel.load_schema(cx);
        // Keep the relative "loaded" label fresh.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        panel
    }

    /// Debounced background lint; results land as editor diagnostics.
    fn schedule_lint(&mut self, cx: &mut Context<Self>) {
        self.lint_seq += 1;
        let seq = self.lint_seq;
        let sql = self.current_sql(cx);
        let engine = self.source.engine();
        let cache = self.schema.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            if this.update(cx, |p, _| p.lint_seq != seq).unwrap_or(true) {
                return;
            }
            let text = sql.clone();
            let lints = cx
                .background_spawn(async move { lint(&text, engine, &cache) })
                .await;
            let _ = this.update(cx, |p, cx| p.apply_lints(seq, &sql, lints, cx));
        })
        .detach();
    }

    fn apply_lints(&mut self, seq: u64, sql: &str, lints: Vec<Lint>, cx: &mut Context<Self>) {
        if self.lint_seq != seq || self.current_sql(cx) != sql {
            return;
        }
        self.sql_input.update(cx, |state, cx| {
            let rope = state.text().clone();
            let Some(set) = state.diagnostics_mut() else {
                return;
            };
            set.reset(&rope);
            for l in lints {
                let severity = match l.severity {
                    Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
                    Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                };
                set.push(lsp_types::Diagnostic {
                    range: lsp_types::Range::new(
                        rope.offset_to_position(l.start),
                        rope.offset_to_position(l.end),
                    ),
                    severity: Some(severity),
                    source: Some("based".into()),
                    message: l.message,
                    ..Default::default()
                });
            }
            cx.notify();
        });
    }

    fn load_schema(&self, cx: &mut Context<Self>) {
        let Some(load) = self.source.load_schema() else {
            return;
        };
        let sql_input = self.sql_input.clone();
        let ai = Some(InlineAi {
            conn: self.conn_id.clone(),
            engine: self.source.engine(),
        });
        let empty = Arc::new(SchemaCache::new(self.source.engine()));
        sql_editor::attach_sql_completion(&sql_input, empty, ai.clone(), cx);
        cx.spawn(async move |this, cx| {
            let Ok(cache) = db::run(cx, load).await else {
                return;
            };
            let cache = Arc::new(cache);
            cx.update(|cx| sql_editor::attach_sql_completion(&sql_input, cache.clone(), ai, cx));
            let _ = this.update(cx, |p, cx| {
                p.schema = cache;
                p.schedule_lint(cx);
            });
        })
        .detach();
    }

    pub(crate) fn connection_id(&self) -> &ConnectionId {
        &self.conn_id
    }

    /// Replace the editor text (palette history, saved queries).
    pub fn set_sql(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        set_input_text(&self.sql_input, sql, window, cx);
        self.dirty = true;
        mark_tab_dirty(cx.entity_id(), cx);
    }

    pub fn current_sql(&self, cx: &App) -> String {
        text_from_input(&self.sql_input, cx)
    }

    /// Pretty-print the editor contents.
    fn format(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sql = self.current_sql(cx);
        if sql.trim().is_empty() {
            return;
        }
        set_input_text(&self.sql_input, &based_query::format_sql(&sql), window, cx);
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

    pub(super) fn run(&mut self, cx: &mut Context<Self>) {
        let sql_raw = self.current_sql(cx);
        if sql_raw.trim().is_empty() {
            return;
        }
        if self.gate_ai_run(&sql_raw, cx) {
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
                            finished_at: OffsetDateTime::now_utc(),
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
            finished_at,
        } => QueryStatusDisplay::Done {
            rows: *rows,
            affected: *affected,
            elapsed_ms: *elapsed_ms,
            finished_at: *finished_at,
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
                    Some(
                        Button::new(format!("{}-fix-ai", self.source.id()))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Bot)
                            .label("Fix with AI")
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.fix_with_ai(window, cx);
                            }))
                            .into_any_element(),
                    ),
                    cx,
                ))
                .into_any_element(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
                ..
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
                Button::new(format!("{id}-format"))
                    .ghost()
                    .small()
                    .label("Format")
                    .on_click(cx.listener(|panel, _, window, cx| panel.format(window, cx))),
            )
            .child(
                Button::new(format!("{id}-explain"))
                    .ghost()
                    .small()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.switch_to_explain(cx))),
            )
            .child(
                Button::new(format!("{id}-ai"))
                    .ghost()
                    .small()
                    .icon(IconName::Bot)
                    .label("Ask AI")
                    .selected(self.ai.is_open())
                    .on_click(cx.listener(|panel, _, window, cx| panel.toggle_ai(window, cx))),
            )
            .child(self.assist_menu(cx))
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.source.id();
        let has_error = matches!(self.status, QueryStatus::Error(_));
        let toolbar = self.render_toolbar(cx);
        let ai_confirm = self.render_ai_confirm(cx);
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
                .on_action(cx.listener(|panel, _: &AskAi, window, cx| panel.toggle_ai(window, cx)))
                .child(toolbar)
                .when_some(self.ai.bar(), |v, bar| v.child(bar))
                .children(ai_confirm)
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
