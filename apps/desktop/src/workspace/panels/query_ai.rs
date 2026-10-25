//! AI prompt bar wiring for [`QueryEditorPanel`]: open/close, context, applying proposals.

use based_ai::context::ContextLevel;
use based_project::AiSettings;
use gpui_kit::{App, AppContext as _, Context, Entity, Window};

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
