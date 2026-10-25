//! Conversational assistant shown in the workspace side pane.

mod view;

use std::mem::take;

use based_ai::{AiError, Message, prompt};
use based_core::EngineKind;
use gpui_kit::component::{
    input::{InputEvent, TextareaState},
    message_scroller::MessageScrollerState,
};
use gpui_kit::{AppContext as _, Context, Entity, SharedString, Task, Window};

use super::launch::launch;
use super::stream::{AiStream, StreamEvent};

/// The connection the conversation is about.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatContext {
    pub engine: EngineKind,
    pub label: String,
}

enum Phase {
    Idle,
    /// A reply is being generated; it occupies the last row.
    Streaming,
    /// The last request failed; the error occupies the last row.
    Failed(AiError),
}

pub struct ChatPane {
    input: Entity<TextareaState>,
    scroller: Entity<MessageScrollerState>,
    turns: Vec<Message>,
    /// Reply text received so far.
    draft: String,
    phase: Phase,
    /// Rows the scroller has been told about.
    shown: usize,
    ctx: Option<ChatContext>,
    /// Why chat can't be used right now; replaces the composer.
    notice: Option<SharedString>,
    /// Provider key lookup, then the stream. Dropping either cancels the request.
    job: Option<Task<()>>,
    stream: Option<AiStream>,
}

impl ChatPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .submit_on_enter(true)
                .placeholder("Ask about your data...")
        });
        cx.subscribe_in(
            &input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().trim().to_owned();
                    if !text.is_empty() && this.send(text, cx) {
                        input.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                }
            },
        )
        .detach();
        Self {
            input,
            scroller: cx.new(|cx| MessageScrollerState::new(0, cx)),
            turns: Vec::new(),
            draft: String::new(),
            phase: Phase::Idle,
            shown: 0,
            ctx: None,
            notice: None,
            job: None,
            stream: None,
        }
    }

    /// Called on every workspace render; only notifies on a real change.
    pub fn set_context(
        &mut self,
        ctx: Option<ChatContext>,
        notice: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        if self.ctx != ctx || self.notice != notice {
            self.ctx = ctx;
            self.notice = notice;
            cx.notify();
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| state.focus(window, cx));
    }

    fn streaming(&self) -> bool {
        matches!(self.phase, Phase::Streaming)
    }

    /// Conversation turns plus the live row while a reply is pending or failed.
    fn rows(&self) -> usize {
        self.turns.len() + usize::from(!matches!(self.phase, Phase::Idle))
    }

    /// Tells the scroller how the row count changed and re-measures the growing last row.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows();
        let shown = self.shown;
        self.shown = rows;
        self.scroller.update(cx, |scroller, cx| {
            if rows > shown {
                scroller.append(rows - shown, cx);
            } else if rows < shown {
                scroller.reset(rows, cx);
            } else if rows > 0 {
                scroller.remeasure_items(rows - 1..rows, cx);
            }
        });
        cx.notify();
    }

    /// `true` when the message was accepted.
    fn send(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        if self.streaming() || self.notice.is_some() {
            return false;
        }
        self.turns.push(Message::user(text));
        self.draft.clear();
        self.phase = Phase::Streaming;
        let history = self.turns.clone();
        let (engine, label) = self
            .ctx
            .as_ref()
            .map_or((None, String::new()), |c| (Some(c.engine), c.label.clone()));
        self.job = Some(launch(
            cx,
            move |model| prompt::chat_request(model, engine, &label, history),
            |this, stream| this.stream = Some(stream),
            Self::on_event,
        ));
        self.sync(cx);
        true
    }

    fn on_event(&mut self, event: StreamEvent, cx: &mut Context<Self>) {
        match event {
            StreamEvent::Text(text) => self.draft.push_str(&text),
            StreamEvent::Done(_) => {
                let reply = take(&mut self.draft);
                self.turns.push(Message::assistant(reply));
                self.finish(Phase::Idle);
            }
            StreamEvent::Failed(e) => self.finish(Phase::Failed(e)),
        }
        self.sync(cx);
    }

    fn finish(&mut self, phase: Phase) {
        self.phase = phase;
        self.job = None;
        self.stream = None;
    }

    /// Cancels the running request, keeping what was received as a reply.
    fn stop(&mut self, cx: &mut Context<Self>) {
        if !self.streaming() {
            return;
        }
        let partial = take(&mut self.draft);
        if !partial.is_empty() {
            self.turns.push(Message::assistant(partial));
        }
        self.finish(Phase::Idle);
        self.sync(cx);
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.turns.clear();
        self.draft.clear();
        self.finish(Phase::Idle);
        self.sync(cx);
    }
}
