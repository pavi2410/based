// SQL query editor shared by SQL engines via [`QuerySource`]: editor, results, messages, explain.

use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

use based_core::{EngineKind, QueryError, ResultSet, SqlOutcome};
use based_project::{SaveTarget, save_sql_query};
use based_query::{
    Lint, RunOutcome, Severity, changes_data, is_destructive, split_statements, statement_at,
    write_summary,
};
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, WindowExt,
    button::{Button, ButtonVariants, DropdownButton},
    dialog::{DialogAction, DialogClose, DialogFooter},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{
        EditorState, GutterDecoration, GutterDecorationCollection, Input, InputEvent, RopeExt as _,
    },
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
use crate::connection::{ConnectionId, Environment};
use crate::db;
use crate::editor::error_locate::{line_col, token_at};
use crate::editor::sql_completion::InlineAi;
use crate::editor::{SchemaCache, lint::lint};
use crate::project::{ProjectRoot, ProjectVars, substitute};
use crate::query_store::{HistoryEntry, QueryStore};
use crate::widgets::column_header::ColumnMeta;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::datetime::format_relative;
use crate::widgets::export_popover::export_popover;
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

mod run;
mod save;
mod source;
mod view;

pub(crate) use save::save_target_for;
pub use source::{Explained, Fut, HoldReason, PlanView, QuerySource};

/// A run held back until the user confirms its banner. Any edit drops it.
/// One statement sent to the server: its text as run, and where that text
/// starts in the editor when it ran exactly as written there.
struct Sent {
    sql: String,
    at: Option<usize>,
}

pub(super) struct HeldRun {
    pub(super) sql: String,
    /// Where `sql` starts in the editor text, when it was taken from there.
    pub(super) base: Option<usize>,
    pub(super) batch: bool,
    /// Runs where the database refuses writes (an assistant card's Run).
    pub(super) read_only: bool,
    pub(super) reason: HoldReason,
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

impl StatementResult {
    /// Grid columns, per-column type metadata, and cells for a returned row set.
    fn from_set(set: ResultSet) -> Self {
        Self {
            columns: set
                .columns
                .iter()
                .map(|c| data_column(c.name.clone(), c.name.clone()))
                .collect(),
            meta: set
                .columns
                .iter()
                .map(|c| {
                    c.type_name
                        .clone()
                        .map(meta_from_query_type)
                        .unwrap_or_default()
                })
                .collect(),
            rows: cells(set.rows),
        }
    }
}

pub struct QueryEditorPanel {
    focus_handle: FocusHandle,
    pub(super) source: Arc<dyn QuerySource>,
    pub(super) conn_id: ConnectionId,
    pub(super) sql_input: Entity<EditorState>,
    /// Gutter bars: the text Run will execute, and the line of the last error.
    gutter: GutterDecorationCollection,
    /// What the gutter shows now, so it is only rewritten when that changes.
    gutter_marks: Vec<GutterMark>,
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
        Self::new_with_initial(source, conn_id, None, false, window, cx)
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
        let gutter = sql_input.update(cx, |state, cx| {
            state.set_gutter_decorations_width(px(3.0), cx);
            state.create_gutter_decorations_collection(Vec::new(), cx)
        });
        let result = cx.new(|cx| configure_row_table(RowDelegate::default(), window, cx));
        let split_state = cx.new(|_| ResizableState::default());
        let panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            sql_input: sql_input.clone(),
            gutter,
            gutter_marks: Vec::new(),
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
    /// The editor now holds SQL the assistant wrote: running it unchanged asks first
    /// when it may write, as text applied from the prompt bar does.
    pub fn mark_ai_written(&mut self, sql: &str) {
        self.ai.note_applied(sql);
    }

    /// Run the editor's SQL once where the database refuses writes.
    pub fn run_read_only(&mut self, cx: &mut Context<Self>) {
        let text = self.current_sql(cx);
        self.run_statements(text, Some(0), true, false, true, cx);
    }

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

/// One bar in the editor gutter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GutterMark {
    /// The statement or selection Run executes, when it is not the whole editor.
    RunRange(Range<usize>),
    /// The text the last database error points at.
    Error(Range<usize>),
}

impl QueryEditorPanel {
    /// Bring the gutter in line with the run range and the last error. Cheap when
    /// nothing changed, so render can call it.
    pub(super) fn sync_gutter(&mut self, cx: &mut Context<Self>) {
        let mut marks = Vec::new();
        let len = self.sql_input.read(cx).value().len();
        let run = self.run_range(cx);
        if !self.status.is_running() && run != (0..len) {
            marks.push(GutterMark::RunRange(run));
        }
        if let Some(span) = self.status.error_span() {
            marks.push(GutterMark::Error(span));
        }
        if marks == self.gutter_marks {
            return;
        }
        let theme = cx.theme();
        let (accent, danger) = (theme.primary.opacity(0.55), theme.danger);
        let decorations = marks
            .iter()
            .map(|m| match m {
                GutterMark::RunRange(r) => GutterDecoration::new(r.clone()).with_color(accent),
                GutterMark::Error(r) => GutterDecoration::new(r.clone()).with_color(danger),
            })
            .collect();
        self.gutter.set(decorations, cx);
        self.gutter_marks = marks;
    }
    /// The gutter bars as `run a..b` / `error a..b`, for tests.
    pub fn gutter_snapshot(&self) -> Vec<String> {
        self.gutter_marks
            .iter()
            .map(|m| match m {
                GutterMark::RunRange(r) => format!("run {}..{}", r.start, r.end),
                GutterMark::Error(r) => format!("error {}..{}", r.start, r.end),
            })
            .collect()
    }
}
