//! AI prompt bar wiring for [`QueryEditorPanel`]: open/close, context, applying proposals.

use based_ai::context::ContextLevel;
use based_ai::tools::check;
use based_core::EngineKind;
use based_project::AiSettings;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement, Styled, Window,
    div, prelude::*,
};

use super::query_editor::{QueryEditorPanel, QueryStatus};
use crate::ai::{AiPrompt, AiPromptEvent, schema};
use crate::widgets::sql_editor::{set_input_text, text_from_input};
use crate::workspace::WorkspaceRef;

const FIX_LABEL: &str = "Fix the error";
const FIX_PROMPT: &str = "Fix this error without changing what the query does:";
const MAX_ERROR_CHARS: usize = 2000;
const OFF_NOTICE: &str = "AI is off for this connection. Enable it in the connection settings.";

impl QueryEditorPanel {
    pub(super) fn new_ai(window: &mut Window, cx: &mut Context<Self>) -> Entity<AiPrompt> {
        let ai = cx.new(|cx| AiPrompt::new(window, cx));
        cx.subscribe_in(&ai, window, Self::on_ai_event).detach();
        ai
    }

    fn ai_settings(&self, cx: &App) -> Option<AiSettings> {
        let ws = cx.try_global::<WorkspaceRef>()?;
        let registry = ws.0.read(cx).registry().read(cx);
        registry.get(&self.conn_id, cx)?.read(cx).ai
    }

    fn ai_off(&self, cx: &App) -> bool {
        schema::level(self.ai_settings(cx)) == ContextLevel::Off
    }

    /// Opens the bar and asks the model to repair the query from the last error.
    pub(super) fn fix_with_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let QueryStatus::Error(error) = &self.status else {
            return;
        };
        let error: String = error.chars().take(MAX_ERROR_CHARS).collect();
        if !self.ai_open {
            self.toggle_ai(window, cx);
        }
        if self.ai_off(cx) {
            return;
        }
        self.ai
            .update(cx, |ai, cx| ai.set_instruction(FIX_LABEL, window, cx));
        self.ask_ai(format!("{FIX_PROMPT}\n{error}"), cx);
    }

    pub(super) fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_open {
            return self.close_ai(window, cx);
        }
        let off = self.ai_off(cx);
        self.ai.update(cx, |ai, cx| {
            ai.set_notice(off.then(|| OFF_NOTICE.into()), cx)
        });
        self.ai_open = true;
        self.ai.update(cx, |ai, cx| ai.focus(window, cx));
        cx.notify();
    }

    fn close_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ai.update(cx, |ai, cx| ai.reset(cx));
        self.ai_open = false;
        self.sql_input.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    fn on_ai_event(
        &mut self,
        _: &Entity<AiPrompt>,
        event: &AiPromptEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            AiPromptEvent::Submit(instruction) => self.ask_ai(instruction.clone(), cx),
            AiPromptEvent::Apply(text) => {
                self.ai_applied = Some(text.clone());
                self.ai_confirm = false;
                set_input_text(&self.sql_input, text, window, cx);
                self.close_ai(window, cx);
            }
            AiPromptEvent::Close => self.close_ai(window, cx),
        }
    }

    fn ask_ai(&mut self, instruction: String, cx: &mut Context<Self>) {
        let engine = self.source.engine();
        let current = text_from_input(&self.sql_input, cx);
        let level = schema::level(self.ai_settings(cx));
        let mentioned = format!("{current} {instruction}");
        let schema = schema::context(engine, &self.schema, level, &mentioned);
        self.ai.update(cx, |ai, cx| {
            ai.start(engine, schema, current, instruction, cx)
        });
    }
}

/// AI-written text that writes or changes schema must be confirmed before its first run.
/// Text the user edited after applying is theirs and runs normally.
fn needs_confirm(engine: EngineKind, applied: Option<&str>, current: &str) -> bool {
    applied.is_some_and(|a| a.trim() == current.trim()) && check(engine, current).is_err()
}

impl QueryEditorPanel {
    /// `true` when the run was held back for confirmation.
    pub(super) fn gate_ai_run(&mut self, sql: &str, cx: &mut Context<Self>) -> bool {
        let hold = needs_confirm(self.source.engine(), self.ai_applied.as_deref(), sql);
        if self.ai_confirm != hold {
            self.ai_confirm = hold;
            cx.notify();
        }
        hold
    }

    fn confirm_ai_run(&mut self, cx: &mut Context<Self>) {
        self.ai_applied = None;
        self.ai_confirm = false;
        self.run(cx);
    }

    pub(super) fn render_ai_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ai_confirm {
            return None;
        }
        let id = self.source.id();
        let warn = cx.theme().warning;
        Some(
            h_flex()
                .gap_2()
                .px_2()
                .py_1p5()
                .items_center()
                .border_b_1()
                .border_color(warn.opacity(0.4))
                .bg(warn.opacity(0.08))
                .child(Icon::new(IconName::TriangleAlert).small().text_color(warn))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .child("This query came from AI and may change data or schema. Run it?"),
                )
                .child(
                    Button::new(format!("{id}-ai-run"))
                        .primary()
                        .small()
                        .label("Run anyway")
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_ai_run(cx))),
                )
                .child(
                    Button::new(format!("{id}-ai-cancel"))
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_confirm = false;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG: EngineKind = EngineKind::Postgres;

    #[test]
    fn unchanged_ai_write_needs_confirmation() {
        assert!(needs_confirm(PG, Some("DELETE FROM t"), "DELETE FROM t\n"));
        assert!(needs_confirm(PG, Some("DROP TABLE t"), "DROP TABLE t"));
    }

    #[test]
    fn ai_reads_run_freely() {
        assert!(!needs_confirm(PG, Some("SELECT 1"), "SELECT 1"));
    }

    #[test]
    fn edited_or_manual_text_is_not_gated() {
        assert!(!needs_confirm(
            PG,
            Some("DELETE FROM t"),
            "DELETE FROM t WHERE id = 1"
        ));
        assert!(!needs_confirm(PG, None, "DELETE FROM t"));
    }

    #[test]
    fn redis_writes_are_gated() {
        assert!(needs_confirm(EngineKind::Redis, Some("DEL k"), "DEL k"));
        assert!(!needs_confirm(EngineKind::Redis, Some("GET k"), "GET k"));
    }
}
