//! Running SQL: holds and confirmations, statement batches, results, errors.

use super::*;
use crate::workspace::connection_health::mark_connection_lost;

/// Row cap for an assistant card's read-only run.
const READ_ONLY_ROWS: usize = 1000;

impl QueryEditorPanel {
    /// What Run executes: the selection, else the statement under the cursor when the
    /// editor holds several, else everything. Returned as its range in the editor text.
    pub(super) fn run_range(&self, cx: &App) -> Range<usize> {
        let state = self.sql_input.read(cx);
        let all: &str = &state.value();
        let selected = state.selected_range();
        if all
            .get(selected.clone())
            .is_some_and(|s| !s.trim().is_empty())
        {
            return selected;
        }
        let engine = self.source.engine();
        if split_statements(all, engine).len() > 1
            && let Some(range) = statement_at(all, engine, state.cursor())
        {
            return range;
        }
        0..all.len()
    }

    /// Run the selection or the current statement.
    pub(in crate::workspace::panels) fn run(&mut self, cx: &mut Context<Self>) {
        let range = self.run_range(cx);
        self.run_statement(range, cx);
    }

    /// Run the whole editor text as one batch.
    pub fn run_all(&mut self, cx: &mut Context<Self>) {
        let text = self.current_sql(cx);
        self.run_statements(text, Some(0), true, false, false, cx);
    }

    /// Run the held text after its banner was confirmed. Confirming the production banner
    /// skips that gate for this one run; confirming the AI banner re-checks production.
    pub(in crate::workspace::panels) fn run_held(
        &mut self,
        skip_production_gate: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(held) = self.held.take() {
            self.run_statements(
                held.sql,
                held.base,
                held.batch,
                skip_production_gate,
                held.read_only,
                cx,
            );
        }
    }

    /// Drop the held run and its banner (the text changed or the user cancelled).
    pub(in crate::workspace::panels) fn clear_hold(&mut self, cx: &mut Context<Self>) {
        if self.held.take().is_some() {
            cx.notify();
        }
    }

    /// Why the last run is waiting behind a banner, if it is.
    pub fn hold_reason(&self) -> Option<HoldReason> {
        self.held.as_ref().map(|h| h.reason)
    }

    /// Run text taken from `base` in the editor (`None` when it isn't editor text).
    pub(super) fn run_sql(&mut self, sql_raw: String, base: Option<usize>, cx: &mut Context<Self>) {
        self.run_statements(sql_raw, base, false, false, false, cx);
    }

    /// Why `sql` must wait for confirmation before it runs, if it must.
    pub(super) fn hold_for(
        &self,
        sql: &str,
        skip_production_gate: bool,
        cx: &Context<Self>,
    ) -> Option<HoldReason> {
        if self.ai.needs_confirm(self.source.engine(), sql) {
            Some(HoldReason::AiWrite)
        } else if !skip_production_gate
            && changes_data(sql, self.source.engine())
            && self.production_name(cx).is_some()
        {
            Some(HoldReason::ProductionWrite)
        } else if !skip_production_gate && is_destructive(sql, self.source.engine()) {
            Some(HoldReason::Destructive)
        } else {
            None
        }
    }

    /// Run `sql_raw` as one statement, or with `batch` split it and run each in order,
    /// stopping at the first failure. Every statement that returns rows gets a result tab.
    pub(super) fn run_statements(
        &mut self,
        sql_raw: String,
        base: Option<usize>,
        batch: bool,
        skip_production_gate: bool,
        read_only: bool,
        cx: &mut Context<Self>,
    ) {
        // One run at a time: a second would abandon the first, which keeps running.
        if sql_raw.trim().is_empty() || self.status.is_running() {
            return;
        }
        if let Some(reason) = self.hold_for(&sql_raw, skip_production_gate, cx) {
            self.held = Some(HeldRun {
                sql: sql_raw,
                base,
                batch,
                read_only,
                reason,
            });
            cx.notify();
            return;
        }
        self.clear_hold(cx);
        let project_vars = self.project_vars(cx);
        let var_ctx = based_query::VariableContext {
            session: Default::default(),
            query: Default::default(),
            collection: Default::default(),
            environment: None,
            workspace: project_vars.clone(),
            connection: project_vars.clone(),
        };
        let texts: Vec<(Option<usize>, String)> = if batch {
            split_statements(&sql_raw, self.source.engine())
                .into_iter()
                .map(|r| (base.map(|b| b + r.start), sql_raw[r].to_string()))
                .collect()
        } else {
            vec![(base, sql_raw)]
        };
        let mut resolved = Vec::with_capacity(texts.len());
        for (at, text) in &texts {
            match based_query::resolve_query(&substitute(text, &project_vars), &var_ctx) {
                // Positions map back to the editor only when the text ran as written.
                Ok(sql) => resolved.push(Sent {
                    at: at.filter(|_| &sql == text),
                    sql,
                }),
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
            for (index, sent) in resolved.into_iter().enumerate() {
                let fut = if read_only {
                    source.run_read_only(sent.sql.clone(), READ_ONLY_ROWS)
                } else {
                    source.run(sent.sql.clone(), timeout)
                };
                let start = Instant::now();
                let outcome = db::run_try(cx, fut, |e| QueryError::new(e.to_string())).await;
                let ms = start.elapsed().as_millis() as u64;
                let proceed = this
                    .update(cx, |panel, cx| {
                        panel.finish_statement(index, total, sent, outcome, ms, cx)
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
    pub(super) fn finish_statement(
        &mut self,
        index: usize,
        total: usize,
        sent: Sent,
        outcome: Result<SqlOutcome, QueryError>,
        ms: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let last = index + 1 == total;
        let conn_id = self.conn_id.clone();
        let Sent { sql, at } = sent;
        let first_line = sql
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        let batch = total > 1;
        let (history_outcome, proceed) = match outcome {
            Ok(out) => {
                let (set, affected) = match out {
                    SqlOutcome::Rows(set) => (set, None),
                    SqlOutcome::Affected(a) => (ResultSet::default(), Some(a)),
                };
                let result = StatementResult::from_set(set);
                let n = result.rows.len();
                let tabular = affected.is_none();
                let what = match affected {
                    None => format!("{n} rows"),
                    Some(a) => format!("{a} rows affected"),
                };
                let run = &mut self.last_run;
                run.log
                    .push(format!("{}  {first_line} — {what} · {ms} ms", index + 1));
                if let Some(a) = affected {
                    run.affected = Some(run.affected.unwrap_or(0) + a);
                }
                if tabular || !batch {
                    run.results.push(result);
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
                (
                    RunOutcome::Ok {
                        rows: Some(n as u64),
                    },
                    true,
                )
            }
            Err(e) => {
                if e.connection_lost {
                    mark_connection_lost(cx.entity_id(), &conn_id, &e.message, cx);
                }
                self.bottom_tab = BottomTab::Messages;
                let message = e.to_string();
                self.last_run
                    .log
                    .push(format!("{}  {first_line} — failed", index + 1));
                let span = self.span_in_editor(&sql, at, e.position, cx);
                let history = RunOutcome::error(&message);
                self.status = QueryStatus::Error { message, span };
                self.mark_error(cx);
                (history, false)
            }
        };
        if let Some(window) = Workspaces::window_for_panel(cx.entity_id(), cx) {
            Workspaces::update_project(window, cx, |p| {
                p.queries.push_history(HistoryEntry::new(
                    conn_id,
                    sql,
                    Duration::from_millis(ms),
                    history_outcome,
                ));
            });
        }
        cx.notify();
        proceed
    }

    /// How many result sets the last run produced.
    pub(in crate::workspace::panels) fn result_count(&self) -> usize {
        self.last_run.results.len()
    }

    /// Which of them the grid shows.
    pub(in crate::workspace::panels) fn active_result(&self) -> usize {
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
    pub(super) fn production_name(&self, cx: &Context<Self>) -> Option<SharedString> {
        let registry = Workspaces::registry_for_panel(cx.entity_id(), cx)?;
        let entry = registry.read(cx).get(&self.conn_id, cx)?.clone();
        let e = entry.read(cx);
        (Environment::from_tags(&e.tags) == Some(Environment::Production))
            .then(|| SharedString::from(e.label.as_str().to_string()))
    }

    /// What the held text does, e.g. "DELETE with no WHERE clause".
    pub(in crate::workspace::panels) fn held_summary(&self) -> String {
        let sql = self.held.as_ref().map_or("", |h| h.sql.as_str());
        let summary = write_summary(sql, self.source.engine());
        if summary.is_empty() {
            "a change".to_owned()
        } else {
            summary
        }
    }

    pub(in crate::workspace::panels) fn confirm_write_run(&mut self, cx: &mut Context<Self>) {
        self.run_held(true, cx);
    }

    /// Run the statement at `range` of the editor text.
    pub(super) fn run_statement(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let text = self.current_sql(cx);
        if let Some(sql) = text.get(range.clone()) {
            self.run_sql(sql.to_string(), Some(range.start), cx);
        }
    }

    /// Where in the editor a failed statement's error points: the token at the
    /// server's position, when `sql` still sits at `at` in the editor text.
    pub(super) fn span_in_editor(
        &self,
        sql: &str,
        at: Option<usize>,
        position: Option<usize>,
        cx: &App,
    ) -> Option<Range<usize>> {
        let (at, position) = (at?, position?);
        let editor = self.current_sql(cx);
        if editor.get(at..at + sql.len()) != Some(sql) {
            return None;
        }
        token_at(&editor, at + position)
    }

    /// Squiggle the editor text the last error points at.
    pub(super) fn mark_error(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn jump_to_error(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(span) = self.status.error_span() else {
            return;
        };
        self.sql_input.update(cx, |state, cx| {
            state.set_selected_range(span, cx);
            state.focus(window, cx);
        });
    }

    /// Stop waiting for the running query. The server may still finish it.
    pub(super) fn stop(&mut self, cx: &mut Context<Self>) {
        let after_ms = self.status.running_ms().unwrap_or(0);
        // Replacing `Running` drops the run and its ticker.
        self.status = QueryStatus::Cancelled { after_ms };
        self.bottom_tab = BottomTab::Messages;
        cx.notify();
    }
}
