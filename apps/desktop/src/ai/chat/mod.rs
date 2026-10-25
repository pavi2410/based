//! Conversational assistant shown in the workspace side pane.
//!
//! One user message can span several model rounds: when the model asks for tools they run
//! read-only against the focused connection and the results go back to it, up to
//! [`MAX_ROUNDS`].

mod view;

use std::mem::take;
use std::sync::Arc;

use based_ai::context::ContextLevel;
use based_ai::tools::{ReadOnlyTools, ToolBackend, ToolCall};
use based_ai::{AiError, Message, Role, prompt};
use based_core::EngineKind;
use gpui_kit::component::{
    input::{InputEvent, TextareaState},
    message_scroller::MessageScrollerState,
};
use gpui_kit::{App, AppContext as _, Context, Entity, SharedString, Task, Window};

use super::launch::launch;
use super::stream::{AiStream, StreamEvent};
use super::tools;
use crate::connection::ConnectionEntry;
use crate::db;

/// Model rounds per user message; the last one is offered no tools so it must answer.
const MAX_ROUNDS: usize = 8;

type Tools = Arc<ReadOnlyTools<Arc<dyn ToolBackend>>>;

/// The connection the conversation is about.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatContext {
    pub entry: Entity<ConnectionEntry>,
    pub engine: EngineKind,
    pub label: String,
    pub level: ContextLevel,
}

enum Phase {
    Idle,
    /// A reply is being generated; it occupies the last row.
    Streaming,
    /// Requested tools are running.
    Tools,
    /// The last request failed; the error occupies the last row.
    Failed(AiError),
}

pub struct ChatPane {
    input: Entity<TextareaState>,
    scroller: Entity<MessageScrollerState>,
    /// Includes assistant tool requests and tool results.
    turns: Vec<Message>,
    /// Reply text received so far this round.
    draft: String,
    /// Tool calls requested this round.
    calls: Vec<ToolCall>,
    round: usize,
    phase: Phase,
    /// Rows the scroller has been told about.
    shown: usize,
    ctx: Option<ChatContext>,
    tools: Option<Tools>,
    /// Why chat can't be used right now; replaces the composer.
    notice: Option<SharedString>,
    /// Provider key lookup, then the stream. Dropping either cancels the request.
    job: Option<Task<()>>,
    stream: Option<AiStream>,
    tool_job: Option<Task<()>>,
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
            calls: Vec::new(),
            round: 0,
            phase: Phase::Idle,
            shown: 0,
            ctx: None,
            tools: None,
            notice: None,
            job: None,
            stream: None,
            tool_job: None,
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

    fn busy(&self) -> bool {
        matches!(self.phase, Phase::Streaming | Phase::Tools)
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

    /// Tools for the focused connection, honouring its `[ai]` context level.
    fn connection_tools(&self, cx: &App) -> Option<Tools> {
        let ctx = self.ctx.as_ref().filter(|c| c.level != ContextLevel::Off)?;
        let backend = tools::backend(ctx.entry.read(cx), cx)?;
        let rows = ctx.level == ContextLevel::SchemaRows;
        Some(Arc::new(ReadOnlyTools::new(backend).with_rows(rows)))
    }

    /// Sends `text` as if typed; `false` when the assistant is busy or unavailable.
    pub fn ask(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        self.send(text, cx)
    }

    /// `true` when the message was accepted.
    fn send(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        if self.busy() || self.notice.is_some() {
            return false;
        }
        self.turns.push(Message::user(text));
        self.tools = self.connection_tools(cx);
        self.round = 0;
        self.begin_round(cx);
        true
    }

    /// Asks the model to continue the conversation.
    fn begin_round(&mut self, cx: &mut Context<Self>) {
        self.draft.clear();
        self.calls.clear();
        self.phase = Phase::Streaming;
        let history = self.turns.clone();
        let (engine, label) = self
            .ctx
            .as_ref()
            .map_or((None, String::new()), |c| (Some(c.engine), c.label.clone()));
        let specs = match &self.tools {
            Some(tools) if self.round < MAX_ROUNDS => tools.specs(),
            _ => Vec::new(),
        };
        self.job = Some(launch(
            cx,
            move |model| prompt::chat_request(model, engine, &label, history, specs),
            |this, stream| this.stream = Some(stream),
            Self::on_event,
        ));
        self.sync(cx);
    }

    fn on_event(&mut self, event: StreamEvent, cx: &mut Context<Self>) {
        match event {
            StreamEvent::Text(text) => self.draft.push_str(&text),
            StreamEvent::Tool(call) => self.calls.push(call),
            StreamEvent::Done(_) => {
                self.job = None;
                self.stream = None;
                let reply = take(&mut self.draft);
                if self.calls.is_empty() {
                    self.turns.push(Message::assistant(reply));
                    self.phase = Phase::Idle;
                } else {
                    let calls = take(&mut self.calls);
                    self.turns
                        .push(Message::assistant_calls(reply, calls.clone()));
                    self.run_tools(calls, cx);
                }
            }
            StreamEvent::Failed(e) => self.finish(Phase::Failed(e)),
        }
        self.sync(cx);
    }

    /// Runs the model's tool calls on Tokio, then starts the next round.
    fn run_tools(&mut self, calls: Vec<ToolCall>, cx: &mut Context<Self>) {
        let Some(tools) = self.tools.clone() else {
            self.finish(Phase::Idle);
            return;
        };
        self.phase = Phase::Tools;
        self.tool_job = Some(cx.spawn(async move |this, cx| {
            let ran = db::run_infallible(cx, async move {
                let mut done = Vec::with_capacity(calls.len());
                for call in calls {
                    let result = tools.call(&call).await;
                    done.push((call, result));
                }
                done
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.tool_job = None;
                match ran {
                    Ok(done) => {
                        this.turns
                            .extend(done.iter().map(|(call, r)| Message::tool(call, r.clone())));
                        this.round += 1;
                        this.begin_round(cx);
                    }
                    Err(e) => {
                        let e = AiError::new(based_ai::AiErrorKind::Other, format!("{e:#}"));
                        this.finish(Phase::Failed(e));
                        this.sync(cx);
                    }
                }
            });
        }));
    }

    fn finish(&mut self, phase: Phase) {
        self.phase = phase;
        self.job = None;
        self.stream = None;
        self.tool_job = None;
    }

    /// Cancels the running request, keeping what was received as a reply.
    fn stop(&mut self, cx: &mut Context<Self>) {
        match self.phase {
            Phase::Streaming => {
                let partial = take(&mut self.draft);
                if !partial.is_empty() {
                    self.turns.push(Message::assistant(partial));
                }
            }
            // The tool request has no results; drop it so the history stays valid.
            Phase::Tools => {
                if self.turns.last().is_some_and(|m| m.role == Role::Assistant) {
                    self.turns.pop();
                }
            }
            Phase::Idle | Phase::Failed(_) => return,
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
