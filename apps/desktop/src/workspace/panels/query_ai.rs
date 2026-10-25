//! AI prompt bar wiring for [`QueryEditorPanel`]: open/close, context, applying proposals.

use based_ai::context::ContextLevel;
use based_project::AiSettings;
use gpui_kit::{App, AppContext as _, Context, Entity, Window};

use super::query_editor::QueryEditorPanel;
use crate::ai::{AiPrompt, AiPromptEvent, schema};
use crate::widgets::sql_editor::{set_input_text, text_from_input};
use crate::workspace::WorkspaceRef;

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

    pub(super) fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_open {
            return self.close_ai(window, cx);
        }
        let off = schema::level(self.ai_settings(cx)) == ContextLevel::Off;
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
