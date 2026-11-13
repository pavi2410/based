// SQL query editor shared by SQL engines via [`QuerySource`]: editor, results, messages, explain.

use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

use based_core::{EngineKind, Value};
use based_project::{SaveTarget, save_sql_query};
use based_query::{Lint, Severity};
use futures::future::BoxFuture;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, Theme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{EditorState, Input, InputEvent, RopeExt as _},
    menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    resizable::{ResizableState, resizable_panel, v_resizable},
    table::{Column, TableState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{App, Entity, prelude::*, *};

use super::query_ai::Assist;
use crate::ai::AiHost;
use crate::app::prefs;
use crate::bindings::AskAi;
use crate::connection::{ConnectionId, Environment, is_connection_read_only};
use crate::db;
use crate::editor::error_locate::{line_col, locate_error};
use crate::editor::sql_completion::InlineAi;
use crate::editor::statements::{split_statements, statement_at};
use crate::editor::write_guard::{changes_data, write_summary};
use crate::editor::{SchemaCache, lint::lint};
use crate::project::{ProjectRoot, ProjectVars, substitute};
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::column_header::ColumnMeta;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::datetime::format_relative;
use crate::widgets::engine_icon;
use crate::widgets::export_popover::export_popover;
use crate::widgets::metadata_pill::env_pill;
use crate::widgets::motion;
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::query_panel_extras;
use crate::widgets::query_status::{QueryStatusDisplay, query_error_card, query_result_meta};
use crate::widgets::result_tabs::{BottomTab, StripTab, result_tab_strip};
use crate::widgets::shortcut_run_kbd_in_primary_button;
use crate::widgets::sql_editor::{self, new_sql_input, replace_input_text, text_from_input};
use crate::widgets::virtual_table::{
    Cell, RowDelegate, cells, data_column, meta_from_query_type, replace_table_data,
};
use crate::widgets::wizard_fields::new_field;
use crate::workspace::notify;
use crate::workspace::pop_out::PopOutWindowTitle;

use crate::workspace::tabs::spec::SavedQueryRef;
use crate::workspace::{
    WorkspaceRef,
    tabs::{clear_tab_dirty, mark_tab_dirty, note_tab_edited},
};

pub type Fut<T> = BoxFuture<'static, anyhow::Result<T>>;

/// Why a run is waiting behind a confirmation banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldReason {
    /// AI-written text that changes data, not yet run or edited.
    AiWrite,
    /// A data-changing statement on a connection tagged production.
    ProductionWrite,
}

/// A run held back until the user confirms its banner. Any edit drops it.
pub(super) struct HeldRun {
    pub(super) sql: String,
    pub(super) batch: bool,
    pub(super) reason: HoldReason,
}

/// Result of running a script: last result set (name, type) + cells, and rows affected if known.
#[derive(Default)]
pub struct RunOutput {
    pub columns: Vec<(String, String)>,
    pub rows: Vec<Vec<Value>>,
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
    /// A run in flight; replacing the status drops (stops) it.
    Running(ActiveRun),
    Done {
        rows: usize,
        affected: Option<u64>,
        elapsed_ms: u64,
        finished_at: OffsetDateTime,
    },
    Error {
        message: String,
        /// Where the error points in the editor text, when the message names a token.
        span: Option<Range<usize>>,
    },
    /// Stopped by the user; the server may still finish the statement.
    Cancelled {
        after_ms: u64,
    },
}

impl QueryStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    /// Time since the run started, while one is running.
    fn running_ms(&self) -> Option<u64> {
        match self {
            Self::Running(run) => Some(run.started.elapsed().as_millis() as u64),
            _ => None,
        }
    }

    pub(super) fn error_span(&self) -> Option<Range<usize>> {
        match self {
            Self::Error { span, .. } => span.clone(),
            _ => None,
        }
    }
}

/// The tasks of a run in flight. Dropping it stops waiting for the result and
/// stops the elapsed-time ticker.
pub struct ActiveRun {
    started: Instant,
    _task: Task<()>,
    _ticker: Task<()>,
}

/// What the last run produced, kept after it finishes.
#[derive(Default)]
struct LastRun {
    /// One entry per statement that returned rows (or the single statement run).
    results: Vec<StatementResult>,
    /// The result the grid shows.
    active: usize,
    /// One line per statement, for the Messages tab.
    log: Vec<String>,
    affected: Option<u64>,
}

enum ExplainView {
    Empty,
    Running,
    Plan(Box<dyn PlanView>),
    Text(String),
}

/// What one statement produced.
#[derive(Clone)]
pub(super) struct StatementResult {
    columns: Vec<Column>,
    meta: Vec<ColumnMeta>,
    rows: Vec<Vec<Cell>>,
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
    /// Byte range the pending AI edit replaces; `None` means the whole query.
    pub(super) ai_scope: Option<Range<usize>>,
    /// The run waiting behind a confirmation banner; at most one banner shows.
    pub(super) held: Option<HeldRun>,
    last_run: LastRun,
    /// The project query this tab edits, so Save overwrites instead of asking again.
    saved: Option<SavedQueryRef>,
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
            ai_scope: None,
            held: None,
            last_run: LastRun::default(),
            saved: None,
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
                panel.clear_hold(cx);
                mark_tab_dirty(cx.entity_id(), cx);
                panel.schedule_lint(cx);
                note_tab_edited(cx);
            }
            _ => {}
        })
        .detach();
        if auto_run && !sql.trim().is_empty() {
            cx.defer_in(window, |panel, _, cx| panel.run_all(cx));
        }
        // Re-place the run marks when the editor scrolls or its text changes.
        cx.observe(&sql_input, |_, _, cx| cx.notify()).detach();
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

    /// Replace the editor text (palette history, saved queries, Format, AI Apply).
    ///
    /// One undoable edit: ⌘Z restores the previous text. Marks the tab dirty and
    /// re-lints, like a typed edit.
    pub fn set_sql(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        replace_input_text(&self.sql_input, sql, window, cx);
        self.dirty = true;
        self.clear_hold(cx);
        mark_tab_dirty(cx.entity_id(), cx);
        self.schedule_lint(cx);
    }

    /// Re-link a restored or reopened tab to its saved query and unsaved state.
    pub fn restore_saved_state(&mut self, saved: Option<&SavedQueryRef>, dirty: bool) {
        if let Some(saved) = saved {
            self.saved = Some(saved.clone());
            self.tab_label = saved.name.clone().into();
        }
        self.dirty = dirty;
    }

    /// The project query this tab was opened from or last saved to.
    pub fn saved_query(&self) -> Option<SavedQueryRef> {
        self.saved.clone()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
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
        let formatted = based_query::format_sql(&sql);
        if formatted != sql {
            self.set_sql(&formatted, window, cx);
        }
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

    /// What Run executes: the selection, else the statement under the cursor when the
    /// editor holds several, else everything.
    fn run_text(&self, cx: &App) -> String {
        let state = self.sql_input.read(cx);
        let selected = state.selected_value().to_string();
        if !selected.trim().is_empty() {
            return selected;
        }
        let all = state.value().to_string();
        if split_statements(&all).len() > 1
            && let Some(range) = statement_at(&all, state.cursor())
        {
            return all[range].to_string();
        }
        all
    }

    /// Run the selection or the current statement.
    pub(super) fn run(&mut self, cx: &mut Context<Self>) {
        let text = self.run_text(cx);
        self.run_sql(text, cx);
    }

    /// Run the whole editor text as one batch.
    pub fn run_all(&mut self, cx: &mut Context<Self>) {
        let text = self.current_sql(cx);
        self.run_statements(text, true, false, cx);
    }

    /// Run the held text after its banner was confirmed. Confirming the production banner
    /// skips that gate for this one run; confirming the AI banner re-checks production.
    pub(super) fn run_held(&mut self, skip_production_gate: bool, cx: &mut Context<Self>) {
        if let Some(held) = self.held.take() {
            self.run_statements(held.sql, held.batch, skip_production_gate, cx);
        }
    }

    /// Drop the held run and its banner (the text changed or the user cancelled).
    pub(super) fn clear_hold(&mut self, cx: &mut Context<Self>) {
        if self.held.take().is_some() {
            cx.notify();
        }
    }

    /// Why the last run is waiting behind a banner, if it is.
    pub fn hold_reason(&self) -> Option<HoldReason> {
        self.held.as_ref().map(|h| h.reason)
    }

    fn run_sql(&mut self, sql_raw: String, cx: &mut Context<Self>) {
        self.run_statements(sql_raw, false, false, cx);
    }

    /// Why `sql` must wait for confirmation before it runs, if it must.
    fn hold_for(&self, sql: &str, skip_production_gate: bool, cx: &App) -> Option<HoldReason> {
        if self.ai.needs_confirm(self.source.engine(), sql) {
            Some(HoldReason::AiWrite)
        } else if !skip_production_gate && changes_data(sql) && self.production_name(cx).is_some() {
            Some(HoldReason::ProductionWrite)
        } else {
            None
        }
    }

    /// Run `sql_raw` as one statement, or with `batch` split it and run each in order,
    /// stopping at the first failure. Every statement that returns rows gets a result tab.
    fn run_statements(
        &mut self,
        sql_raw: String,
        batch: bool,
        skip_production_gate: bool,
        cx: &mut Context<Self>,
    ) {
        if sql_raw.trim().is_empty() {
            return;
        }
        if let Some(reason) = self.hold_for(&sql_raw, skip_production_gate, cx) {
            self.held = Some(HeldRun {
                sql: sql_raw,
                batch,
                reason,
            });
            cx.notify();
            return;
        }
        self.clear_hold(cx);
        let project_vars = cx.global::<ProjectVars>().vars.clone();
        let var_ctx = based_query::VariableContext {
            session: Default::default(),
            query: Default::default(),
            collection: Default::default(),
            environment: None,
            workspace: project_vars.clone(),
            connection: project_vars.clone(),
        };
        let texts: Vec<String> = if batch {
            split_statements(&sql_raw)
                .into_iter()
                .map(|r| sql_raw[r].to_string())
                .collect()
        } else {
            vec![sql_raw]
        };
        let mut resolved = Vec::with_capacity(texts.len());
        for text in &texts {
            match based_query::resolve_query(&substitute(text, &project_vars), &var_ctx) {
                Ok(sql) => resolved.push(sql),
                Err(e) => {
                    self.status = QueryStatus::Error {
                        message: e.to_string(),
                        span: None,
                    };
                    self.bottom_tab = BottomTab::Messages;
                    cx.notify();
                    return;
                }
            }
        }
        self.bottom_tab = BottomTab::Results;
        self.last_run = LastRun::default();
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        let source = self.source.clone();
        let timeout = prefs::query_timeout_secs(cx);
        let total = resolved.len();
        let task = cx.spawn(async move |this, cx| {
            for (index, sql) in resolved.into_iter().enumerate() {
                let fut = source.run(sql.clone(), timeout);
                let start = Instant::now();
                let outcome = db::run(cx, fut).await;
                let ms = start.elapsed().as_millis() as u64;
                let proceed = this
                    .update(cx, |panel, cx| {
                        panel.finish_statement(index, total, sql, outcome, ms, cx)
                    })
                    .unwrap_or(false);
                if !proceed {
                    break;
                }
            }
        });
        self.status = QueryStatus::Running(ActiveRun {
            started: Instant::now(),
            _task: task,
            _ticker: ticker,
        });
    }

    /// Record one statement's outcome. Returns whether the batch should continue.
    fn finish_statement(
        &mut self,
        index: usize,
        total: usize,
        sql: String,
        outcome: anyhow::Result<RunOutput>,
        ms: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let last = index + 1 == total;
        let conn_id = self.conn_id.clone();
        let first_line = sql
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        let batch = total > 1;
        let (history_rows, proceed) = match outcome {
            Ok(out) => {
                let columns: Vec<Column> = out
                    .columns
                    .iter()
                    .map(|(n, _)| data_column(n.clone(), n.clone()))
                    .collect();
                let meta: Vec<ColumnMeta> = out
                    .columns
                    .iter()
                    .map(|(_, t)| meta_from_query_type(t))
                    .collect();
                let rows = cells(out.rows);
                let n = rows.len();
                let tabular = !columns.is_empty();
                let what = match (tabular, out.affected) {
                    (true, _) => format!("{n} rows"),
                    (false, Some(a)) => format!("{a} rows affected"),
                    (false, None) => "done".to_string(),
                };
                let run = &mut self.last_run;
                run.log
                    .push(format!("{}  {first_line} — {what} · {ms} ms", index + 1));
                if let Some(a) = out.affected {
                    run.affected = Some(run.affected.unwrap_or(0) + a);
                }
                if tabular || !batch {
                    run.results.push(StatementResult {
                        columns,
                        meta,
                        rows,
                    });
                    self.show_result(self.last_run.results.len() - 1, cx);
                }
                if last {
                    let shown_rows = self.last_run.results.last().map_or(0, |r| r.rows.len());
                    self.status = QueryStatus::Done {
                        rows: shown_rows,
                        affected: self.last_run.affected,
                        elapsed_ms: self.status.running_ms().unwrap_or(ms),
                        finished_at: OffsetDateTime::now_utc(),
                    };
                    if self.last_run.results.is_empty() {
                        self.bottom_tab = BottomTab::Messages;
                    }
                }
                (Some(n as u64), true)
            }
            Err(e) => {
                self.bottom_tab = BottomTab::Messages;
                let message = e.to_string();
                self.last_run
                    .log
                    .push(format!("{}  {first_line} — failed", index + 1));
                let editor_sql = self.current_sql(cx);
                let span = locate_error(&editor_sql, &message);
                self.status = QueryStatus::Error { message, span };
                self.mark_error(cx);
                (None, false)
            }
        };
        let run_status = if history_rows.is_some() {
            based_query::RunStatus::Ok
        } else {
            based_query::RunStatus::Error
        };
        cx.update_global(|store: &mut QueryStore, _| {
            store.push_history(HistoryEntry::new(
                conn_id,
                sql,
                ms,
                history_rows,
                run_status,
            ));
        });
        cx.notify();
        proceed
    }

    /// How many result sets the last run produced.
    pub(super) fn result_count(&self) -> usize {
        self.last_run.results.len()
    }

    /// Which of them the grid shows.
    pub(super) fn active_result(&self) -> usize {
        self.last_run.active
    }

    /// Show result `index` in the grid.
    pub fn show_result(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(r) = self.last_run.results.get(index).cloned() else {
            return;
        };
        self.last_run.active = index;
        self.result.update(cx, |state, cx| {
            replace_table_data(state, r.columns, r.rows, r.meta, cx);
        });
        cx.notify();
    }

    /// Production connection name when it is marked as production.
    fn production_name(&self, cx: &App) -> Option<SharedString> {
        let ws = cx.try_global::<WorkspaceRef>()?;
        let entry =
            ws.0.read(cx)
                .registry()
                .read(cx)
                .get(&self.conn_id, cx)?
                .clone();
        let e = entry.read(cx);
        (Environment::from_tags(&e.tags) == Some(Environment::Production))
            .then(|| SharedString::from(e.config.label().to_string()))
    }

    /// What the held text does, e.g. "DELETE with no WHERE clause".
    pub(super) fn held_summary(&self) -> String {
        let sql = self.held.as_ref().map_or("", |h| h.sql.as_str());
        let summary = write_summary(sql);
        if summary.is_empty() {
            "a change".to_owned()
        } else {
            summary
        }
    }

    pub(super) fn confirm_write_run(&mut self, cx: &mut Context<Self>) {
        self.run_held(true, cx);
    }

    fn render_write_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.hold_reason() != Some(HoldReason::ProductionWrite) {
            return None;
        }
        let id = self.source.id();
        let name = self.production_name(cx)?;
        let danger = cx.theme().danger;
        Some(
            h_flex()
                .gap_2()
                .px_2()
                .py_1p5()
                .items_center()
                .border_b_1()
                .border_color(danger.opacity(0.4))
                .bg(danger.opacity(0.08))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .small()
                        .text_color(danger),
                )
                .child(div().flex_1().text_sm().child(format!(
                    "This runs {} on {name} (production). Run it?",
                    self.held_summary()
                )))
                .child(
                    Button::new(format!("{id}-write-run"))
                        .danger()
                        .small()
                        .label("Run on production")
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_write_run(cx))),
                )
                .child(
                    Button::new(format!("{id}-write-cancel"))
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.clear_hold(cx))),
                )
                .into_any_element(),
        )
    }

    /// Nothing has run yet and no result is loaded.
    fn shows_start_screen(&self, cx: &App) -> bool {
        matches!(self.status, QueryStatus::Idle) && self.result.read(cx).delegate().rows.is_empty()
    }

    /// First screen of a fresh query tab: how to run, and recent queries to restore.
    fn render_start_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (border, muted, fg, hover) = (
            theme.border,
            theme.muted_foreground,
            theme.foreground,
            theme.muted.opacity(0.4),
        );
        let recent: Vec<(String, String)> = cx
            .try_global::<QueryStore>()
            .map(|store| {
                store
                    .history_for(&self.conn_id)
                    .into_iter()
                    .take(4)
                    .map(|h| {
                        let first_line = h.query.lines().next().unwrap_or("").trim().to_string();
                        let meta = format!(
                            "{} · {} ms",
                            format_relative(h.ran_at, OffsetDateTime::now_utc()),
                            h.duration_ms
                        );
                        (first_line, meta)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let full: Vec<String> = cx
            .try_global::<QueryStore>()
            .map(|store| {
                store
                    .history_for(&self.conn_id)
                    .into_iter()
                    .take(4)
                    .map(|h| h.query.clone())
                    .collect()
            })
            .unwrap_or_default();
        let id = self.source.id();
        v_flex()
            .flex_1()
            .min_h(px(0.0))
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Run a statement to see results"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Press the Run button, or Cmd-Enter in the editor."),
                    ),
            )
            .when(!recent.is_empty(), |v| {
                v.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("RECENT ON THIS CONNECTION"),
                        )
                        .child(
                            v_flex()
                                .max_w(px(560.0))
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(border)
                                .overflow_hidden()
                                .children(recent.into_iter().enumerate().map(
                                    |(i, (line, meta))| {
                                        let sql = full[i].clone();
                                        h_flex()
                                            .id(SharedString::from(format!("{id}-recent-{i}")))
                                            .gap_3()
                                            .px_3()
                                            .py_1p5()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(i > 0, |r| r.border_t_1().border_color(border))
                                            .hover(move |s| s.bg(hover))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_xs()
                                                    .font_family(prefs::code_font_family(cx))
                                                    .text_color(fg)
                                                    .child(line),
                                            )
                                            .child(div().text_xs().text_color(muted).child(meta))
                                            .on_click(cx.listener(move |panel, _, window, cx| {
                                                panel.set_sql(&sql, window, cx);
                                            }))
                                    },
                                )),
                        ),
                )
            })
            .into_any_element()
    }

    /// Where a saved copy of this query should run.
    fn save_target(&self, cx: &App) -> SaveTarget {
        save_target_for(&self.conn_id, self.source.engine(), cx)
    }

    /// Save the editor text into the project: in place if it was saved before, else
    /// after asking for a name.
    fn save_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = cx.try_global::<ProjectRoot>().map(|r| r.0.clone()) else {
            notify::push_info(cx, "Open a project to save queries.");
            return;
        };
        let sql = self.current_sql(cx);
        if sql.trim().is_empty() {
            return;
        }
        if let Some(saved) = self.saved.clone() {
            self.write_saved(&root, &saved.name, &sql, Some(&saved.path), cx);
            return;
        }
        let first_comment = sql
            .lines()
            .next()
            .and_then(|l| l.trim().strip_prefix("--"))
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
        let name_input = new_field(
            window,
            cx,
            first_comment.as_deref().unwrap_or("Untitled query"),
            "Query name",
        );
        let panel = cx.entity().downgrade();
        let field = name_input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (panel, root, field) = (panel.clone(), root.clone(), field.clone());
            dialog
                .title("Save query")
                .child(Input::new(&name_input).aria_label("Query name").w_full())
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new()
                                .child(Button::new("save-query-cancel").outline().label("Cancel")),
                        )
                        .child(
                            DialogAction::new()
                                .child(Button::new("save-query-ok").primary().label("Save")),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let name = field.read(cx).value().trim().to_string();
                    if name.is_empty() {
                        return false;
                    }
                    let (root, panel) = (root.clone(), panel.clone());
                    let _ = panel.update(cx, |p, cx| {
                        let sql = p.current_sql(cx);
                        p.write_saved(&root, &name, &sql, None, cx);
                    });
                    true
                })
        });
    }

    fn write_saved(
        &mut self,
        root: &Path,
        name: &str,
        sql: &str,
        overwrite: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let target = self.save_target(cx);
        match save_sql_query(root, name, sql, &target, overwrite) {
            Ok(path) => {
                self.saved = Some(SavedQueryRef {
                    path,
                    name: name.to_string(),
                });
                self.tab_label = name.to_string().into();
                self.dirty = false;
                clear_tab_dirty(cx.entity_id(), cx);
                notify::push_info(cx, format!("Saved {name}"));
                cx.notify();
            }
            Err(e) => notify::push_error(cx, "Could not save query", e.to_string()),
        }
    }

    /// Past runs on this connection, newest first: (first line, age and time, full SQL, ok).
    fn history_items(&self, cx: &App) -> Vec<(String, String, String, bool)> {
        let Some(store) = cx.try_global::<QueryStore>() else {
            return Vec::new();
        };
        let now = OffsetDateTime::now_utc();
        store
            .history_for(&self.conn_id)
            .into_iter()
            .take(15)
            .map(|h| {
                let line = h.query.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                let mut line = line.trim().to_string();
                if line.chars().count() > 56 {
                    line = line.chars().take(55).collect::<String>() + "…";
                }
                (
                    line,
                    format!("{} · {} ms", format_relative(h.ran_at, now), h.duration_ms),
                    h.query.clone(),
                    h.status == based_query::RunStatus::Ok,
                )
            })
            .collect()
    }

    /// Toolbar menu that restores a past run into the editor.
    fn history_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.source.id();
        let items = self.history_items(cx);
        let panel = cx.weak_entity();
        Button::new(format!("{id}-history"))
            .ghost()
            .small()
            .label("History")
            .dropdown_caret(true)
            .disabled(items.is_empty())
            .tooltip("Past runs on this connection")
            .dropdown_menu(move |menu, _, _| {
                items.iter().fold(menu, |menu, (line, meta, sql, ok)| {
                    let (panel, sql) = (panel.clone(), sql.clone());
                    menu.item(
                        PopupMenuItem::new(format!("{line}    {meta}"))
                            .icon(if *ok {
                                IconName::Check
                            } else {
                                IconName::TriangleAlert
                            })
                            .on_click(move |_, window, cx| {
                                let _ = panel.update(cx, |p, cx| p.set_sql(&sql, window, cx));
                            }),
                    )
                })
            })
    }

    /// Run the statement at `range` of the editor text.
    fn run_statement(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let text = self.current_sql(cx);
        if let Some(sql) = text.get(range) {
            self.run_sql(sql.to_string(), cx);
        }
    }

    /// A run mark beside the first line of every statement, placed from the editor's
    /// last layout (scroll offset, line height, wrapped rows).
    fn render_run_marks(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.status.is_running() {
            return Vec::new();
        }
        let st = self.sql_input.read(cx);
        let (Some(text_bounds), Some(line_height)) = (st.text_bounds(), st.line_height()) else {
            return Vec::new();
        };
        // `text_bounds` already moves with the scroll offset, so it is not added again.
        let view = st.input_bounds();
        let left = st.input_bounds().origin.x + px(3.0);
        let text = st.value().to_string();
        let marks: Vec<(usize, Range<usize>, Pixels)> = split_statements(&text)
            .into_iter()
            .enumerate()
            .filter_map(|(i, range)| {
                let line = text[..range.start].matches('\n').count();
                let row = st.display_map().buffer_line_to_display_row(line);
                let y = text_bounds.origin.y + line_height * row as f32;
                let inside =
                    y >= view.top() - px(1.0) && y + line_height <= view.bottom() + px(1.0);
                inside.then_some((i, range, y))
            })
            .collect();
        let (success, hover) = (cx.theme().success_foreground, cx.theme().muted.opacity(0.6));
        let id = self.source.id();
        marks
            .into_iter()
            .map(|(i, range, y)| {
                deferred(
                    anchored()
                        .position(point(left, y + (line_height - px(16.0)) / 2.0))
                        .anchor(Anchor::TopLeft)
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-run-mark-{i}")))
                                .size(px(16.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .tooltip(|window, cx| {
                                    Tooltip::new("Run this statement").build(window, cx)
                                })
                                .child(Icon::new(IconName::Play).xsmall().text_color(success))
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    panel.run_statement(range.clone(), cx);
                                })),
                        ),
                )
                .with_priority(500)
                .into_any_element()
            })
            .collect()
    }

    /// Squiggle the editor text the last error points at.
    fn mark_error(&mut self, cx: &mut Context<Self>) {
        let Some(span) = self.status.error_span() else {
            return;
        };
        self.sql_input.update(cx, |state, cx| {
            let rope = state.text().clone();
            let Some(set) = state.diagnostics_mut() else {
                return;
            };
            set.push(lsp_types::Diagnostic {
                range: lsp_types::Range::new(
                    rope.offset_to_position(span.start),
                    rope.offset_to_position(span.end),
                ),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                source: Some("database".into()),
                message: "The database rejected this".into(),
                ..Default::default()
            });
            cx.notify();
        });
    }

    /// Select the text the last error points at and focus the editor.
    fn jump_to_error(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(span) = self.status.error_span() else {
            return;
        };
        self.sql_input.update(cx, |state, cx| {
            state.set_selected_range(span, cx);
            state.focus(window, cx);
        });
    }

    /// Stop waiting for the running query. The server may still finish it.
    fn stop(&mut self, cx: &mut Context<Self>) {
        let after_ms = self.status.running_ms().unwrap_or(0);
        // Replacing `Running` drops the run and its ticker.
        self.status = QueryStatus::Cancelled { after_ms };
        self.bottom_tab = BottomTab::Messages;
        cx.notify();
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
        QueryStatus::Running(_) => QueryStatusDisplay::Running,
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
        QueryStatus::Error { message, .. } => QueryStatusDisplay::Error(message.clone().into()),
        QueryStatus::Cancelled { after_ms } => QueryStatusDisplay::Cancelled {
            after_ms: *after_ms,
        },
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
            BottomTab::Results if self.shows_start_screen(cx) => self.render_start_screen(cx),
            BottomTab::Results => {
                let running = self.status.is_running();
                let empty = self.result.read(cx).delegate().rows.is_empty();
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.0))
                    .when(running && !empty, |d| d.opacity(0.55))
                    .child(render_row_table(&self.result, cx))
                    .when(running && empty, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .child(motion::skeleton_table("qe-skel", 6, 4, cx)),
                        )
                    })
                    .into_any_element()
            }
            BottomTab::Messages => self.render_messages(cx),
            BottomTab::Explain => self.render_explain(cx),
        }
    }

    fn render_messages(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match &self.status {
            QueryStatus::Error { message: full, .. } => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .child(query_error_card(
                    format!("{}-query-error-card", self.source.id()),
                    full.clone().into(),
                    Some(self.error_actions(full, cx)),
                    cx,
                ))
                .into_any_element(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
                ..
            } => {
                if self.last_run.log.len() > 1 {
                    return v_flex()
                        .flex_1()
                        .min_h(px(0.0))
                        .p_3()
                        .gap_1()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_xs()
                        .text_color(muted)
                        .children(self.last_run.log.iter().map(|l| div().child(l.clone())))
                        .into_any_element();
                }
                let affected = affected.map_or(String::new(), |a| format!(" · {a} affected"));
                hint(
                    format!("Query OK · {rows} rows{affected} · {elapsed_ms} ms"),
                    muted,
                )
            }
            QueryStatus::Running(_) => hint("Running…", muted),
            QueryStatus::Cancelled { after_ms } => hint(
                format!(
                    "Stopped after {:.1} s. The server may still finish the statement; nothing is rolled back.",
                    *after_ms as f32 / 1000.0
                ),
                muted,
            ),
            QueryStatus::Idle => hint("No messages yet. Run a query to see output here.", muted),
        }
    }

    /// Location text plus Jump to error and Fix with AI for the error card.
    fn error_actions(&self, full: &str, cx: &mut Context<Self>) -> AnyElement {
        let id = self.source.id();
        let muted = cx.theme().muted_foreground;
        let error_span = self.status.error_span();
        let place = error_span.as_ref().map(|span| {
            let (line, col) = line_col(&self.current_sql(cx), span.start);
            format!("Line {line}, column {col}")
        });
        let _ = full;
        h_flex()
            .gap(px(8.0))
            .items_center()
            .children(place.map(|p| div().text_xs().text_color(muted).child(p)))
            .when(error_span.is_some(), |h| {
                h.child(
                    Button::new(format!("{id}-jump-error"))
                        .ghost()
                        .xsmall()
                        .label("Jump to error")
                        .on_click(cx.listener(|panel, _, window, cx| {
                            panel.jump_to_error(window, cx);
                        })),
                )
            })
            .child(
                Button::new(format!("{id}-fix-ai"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Bot)
                    .label("Fix with AI")
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.fix_with_ai(window, cx);
                    })),
            )
            .into_any_element()
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

    /// Where this editor runs: connection, environment and access, always visible.
    fn context_chip(&self, read_only: bool, cx: &mut Context<Self>) -> AnyElement {
        let entry = cx.try_global::<WorkspaceRef>().and_then(|ws| {
            ws.0.read(cx)
                .registry()
                .read(cx)
                .get(&self.conn_id, cx)
                .cloned()
        });
        let Some(entry) = entry else {
            return div().into_any_element();
        };
        let (label, env) = {
            let e = entry.read(cx);
            (
                SharedString::from(e.config.label().to_string()),
                Environment::from_tags(&e.tags),
            )
        };
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        h_flex()
            .gap(px(7.0))
            .items_center()
            .px(px(9.0))
            .py(px(3.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(border)
            .bg(theme.background)
            .child(engine_icon(self.source.engine()))
            .child(div().text_xs().font_weight(FontWeight::MEDIUM).child(label))
            .children(env.map(|e| env_pill(e, cx)))
            .when(read_only, |h| {
                h.child(
                    div()
                        .id("qe-read-only")
                        .text_xs()
                        .text_color(muted)
                        .child("read-only")
                        .tooltip(|window, cx| {
                            Tooltip::new("Writes will fail on this connection").build(window, cx)
                        }),
                )
            })
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = self.source.id();
        let read_only = cx
            .try_global::<WorkspaceRef>()
            .map(|ws| is_connection_read_only(&self.conn_id, ws.0.read(cx).registry().read(cx), cx))
            .unwrap_or(false);
        let context = self.context_chip(read_only, cx);
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
            .child(context)
            .child(
                div()
                    .w(px(1.0))
                    .h(px(18.0))
                    .mx(px(2.0))
                    .bg(cx.theme().border),
            )
            .child(if self.status.is_running() {
                Button::new(format!("{id}-stop"))
                    .danger()
                    .small()
                    .icon(IconName::Close)
                    .label("Stop")
                    .on_click(cx.listener(|panel, _, _, cx| panel.stop(cx)))
            } else {
                Button::new(format!("{id}-run"))
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label("Run")
                    .child(shortcut_run_kbd_in_primary_button(cx))
                    .on_click(cx.listener(|panel, _, _, cx| panel.run(cx)))
            })
            .when(!self.status.is_running(), |h| {
                let panel = cx.weak_entity();
                h.child(
                    Button::new(format!("{id}-run-more"))
                        .primary()
                        .small()
                        .ml(px(-5.0))
                        .icon(IconName::ChevronDown)
                        .tooltip("Run options")
                        .dropdown_menu(move |menu, _, _| {
                            let (a, b) = (panel.clone(), panel.clone());
                            menu.item(
                                PopupMenuItem::new("Run Selection or Current Statement")
                                    .icon(IconName::Play)
                                    .on_click(move |_, _, cx| {
                                        let _ = a.update(cx, |p, cx| p.run(cx));
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Run All Statements")
                                    .icon(IconName::Play)
                                    .on_click(move |_, _, cx| {
                                        let _ = b.update(cx, |p, cx| p.run_all(cx));
                                    }),
                            )
                        }),
                )
            })
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
            .child(self.history_menu(cx))
            .child(
                Button::new(format!("{id}-save"))
                    .small()
                    .label("Save")
                    .on_click(cx.listener(|panel, _, window, cx| panel.save_query(window, cx))),
            )
            .child(div().flex_1())
            .child(query_panel_extras::variables_popover(
                format!("{id}-vars-popover"),
                project_dir,
                var_map,
                mono_font,
                cx,
            ))
    }
}

impl Render for QueryEditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.source.id();
        let has_error = matches!(self.status, QueryStatus::Error { .. });
        let toolbar = self.render_toolbar(cx);
        let ai_confirm = self.render_ai_confirm(cx);
        let write_confirm = self.render_write_confirm(cx);
        let run_marks = self.render_run_marks(cx);
        let editor_pane = div().size_full().p_2().child(sql_editor::code_editor_flex(
            &self.sql_input,
            has_error,
            false,
            cx,
        ));

        let on_select: Rc<dyn Fn(StripTab, &mut Window, &mut App)> = {
            let entity = cx.entity();
            Rc::new(move |tab, _, cx| {
                entity.update(cx, |panel, cx| {
                    match tab {
                        StripTab::Result(i) => {
                            panel.bottom_tab = BottomTab::Results;
                            panel.show_result(i, cx);
                        }
                        StripTab::Messages => panel.bottom_tab = BottomTab::Messages,
                        StripTab::Explain => panel.bottom_tab = BottomTab::Explain,
                    }
                    cx.notify();
                });
            })
        };
        let (headers, rows): (Vec<String>, Vec<Vec<Cell>>) = {
            let d = self.result.read(cx).delegate();
            (
                d.columns.iter().map(|c| c.key.to_string()).collect(),
                d.rows.clone(),
            )
        };
        let result_counts: Vec<usize> = match (self.last_run.results.len(), &self.status) {
            (0, QueryStatus::Done { rows, .. }) if *rows > 0 => vec![*rows],
            _ => self.last_run.results.iter().map(|r| r.rows.len()).collect(),
        };
        let trailing = h_flex()
            .gap(px(10.0))
            .items_center()
            .child(query_result_meta(
                &status_display(&self.status),
                self.status.running_ms(),
                cx,
            ))
            .children(self.can_ask_about_results(cx).then(|| {
                Button::new(format!("{id}-ask-results"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Bot)
                    .label("Ask about results")
                    .tooltip("Send the first rows to the assistant")
                    .on_click(cx.listener(|panel, _, _, cx| panel.assist(Assist::Results, cx)))
            }))
            .child(export_popover(&format!("{id}-qe"), headers, rows))
            .into_any_element();
        let strip = result_tab_strip(
            &format!("{id}-bt"),
            self.bottom_tab,
            has_error,
            result_counts,
            self.last_run.active,
            Some(trailing),
            on_select,
            cx,
        );
        let bottom_pane = v_flex()
            .size_full()
            .child(strip)
            .child(self.render_bottom_body(cx));

        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, ["Query"], cx);
        let footer = tab_breadcrumb_footer(format!("{id}-qe-breadcrumb"), crumbs, None, cx);

        panel_tab_content(
            v_flex()
                .w_full()
                .h_full()
                .min_h(px(0.0))
                .on_action(cx.listener(|panel, _: &AskAi, window, cx| panel.toggle_ai(window, cx)))
                .children(run_marks)
                .child(toolbar)
                .child(motion::glimmer_bar(
                    format!("{id}-run-glimmer"),
                    self.status.is_running(),
                    cx,
                ))
                .when_some(self.ai.bar(), |v, bar| v.child(bar))
                .children(ai_confirm)
                .children(write_confirm)
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

/// Where a query saved for `conn_id` runs: that connection when it is part of the project,
/// else any connection of its engine.
pub(crate) fn save_target_for(conn_id: &ConnectionId, engine: EngineKind, cx: &App) -> SaveTarget {
    let entry = cx
        .try_global::<WorkspaceRef>()
        .and_then(|ws| ws.0.read(cx).registry().read(cx).get(conn_id, cx).cloned());
    if let Some(entry) = entry {
        let e = entry.read(cx);
        if let Some(key) = e.id.project_key() {
            return SaveTarget::Connection(key);
        }
    }
    SaveTarget::Engine(engine)
}
