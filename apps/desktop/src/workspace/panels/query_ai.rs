//! AI prompt bar wiring for [`QueryEditorPanel`]: context, applying proposals, run confirmation.

use based_ai::context::ContextLevel;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

use super::query_editor::{QueryEditorPanel, QueryStatus};
use crate::ai::{AiPromptEvent, connection_ai, schema};
use crate::widgets::sql_editor::{set_input_text, text_from_input};

const FIX_LABEL: &str = "Fix the error";
const FIX_PROMPT: &str = "Fix this error without changing what the query does:";
const MAX_ERROR_CHARS: usize = 2000;

impl QueryEditorPanel {
    fn ai_level(&self, cx: &gpui_kit::App) -> ContextLevel {
        schema::level(connection_ai(&self.conn_id, cx))
    }

    pub(super) fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let off = self.ai_level(cx) == ContextLevel::Off;
        let was_open = self.ai.is_open();
        self.ai.toggle(off, window, cx);
        if was_open {
            self.focus_editor(window, cx);
        }
    }

    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.sql_input.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Opens the bar and asks the model to repair the query from the last error.
    pub(super) fn fix_with_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let QueryStatus::Error(error) = &self.status else {
            return;
        };
        let error: String = error.chars().take(MAX_ERROR_CHARS).collect();
        if !self.ai.is_open() {
            self.toggle_ai(window, cx);
        }
        if self.ai_level(cx) == ContextLevel::Off {
            return;
        }
        self.ai
            .prompt()
            .update(cx, |ai, cx| ai.set_instruction(FIX_LABEL, window, cx));
        self.ask_ai(format!("{FIX_PROMPT}\n{error}"), cx);
    }

    pub(super) fn on_ai_event(
        &mut self,
        event: &AiPromptEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            AiPromptEvent::Submit(instruction) => self.ask_ai(instruction.clone(), cx),
            AiPromptEvent::Apply(text) => {
                self.ai.note_applied(text);
                self.ai_confirm = false;
                set_input_text(&self.sql_input, text, window, cx);
                self.ai.close(cx);
                self.focus_editor(window, cx);
            }
            AiPromptEvent::Close => {
                self.ai.close(cx);
                self.focus_editor(window, cx);
            }
        }
    }

    fn ask_ai(&mut self, instruction: String, cx: &mut Context<Self>) {
        let engine = self.source.engine();
        let current = text_from_input(&self.sql_input, cx);
        let mentioned = format!("{current} {instruction}");
        let schema = schema::context(engine, &self.schema, self.ai_level(cx), &mentioned);
        self.ai.ask(engine, schema, current, instruction, cx);
    }

    /// `true` when the run was held back for confirmation.
    pub(super) fn gate_ai_run(&mut self, sql: &str, cx: &mut Context<Self>) -> bool {
        let hold = self.ai.needs_confirm(self.source.engine(), sql);
        if self.ai_confirm != hold {
            self.ai_confirm = hold;
            cx.notify();
        }
        hold
    }

    fn confirm_ai_run(&mut self, cx: &mut Context<Self>) {
        self.ai.confirmed();
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
