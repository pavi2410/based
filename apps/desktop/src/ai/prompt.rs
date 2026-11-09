//! Inline AI prompt bar: instruction in, reviewable diff out. Never edits the editor itself;
//! the host applies the proposal after the user accepts it.

use std::mem::take;

use based_ai::{AiError, prompt};
use based_core::EngineKind;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{
    Context, Entity, EventEmitter, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    Styled, Task, Window, div, prelude::*, px,
};

use super::launch::{error_text, launch};
use super::stream::{AiStream, StreamEvent};
use crate::widgets::ai_diff::AiDiff;

/// The entry to show after pressing up (`older`) or down. `None` leaves browsing: past the
/// newest entry, or when there is nothing to recall.
fn step_recall(len: usize, at: Option<usize>, older: bool) -> Option<usize> {
    match (at, older) {
        (None, true) if len > 0 => Some(len - 1),
        (Some(ix), true) => Some(ix.saturating_sub(1)),
        (Some(ix), false) if ix + 1 < len => Some(ix + 1),
        _ => None,
    }
}

/// Instructions kept for recall.
const HISTORY_MAX: usize = 30;

pub enum AiPromptEvent {
    /// The user sent an instruction; the host answers with [`AiPrompt::start`].
    Submit(String),
    /// The user accepted this text.
    Apply(String),
    Close,
}

enum Phase {
    Idle,
    Streaming,
    Proposal,
    Failed(AiError),
}

pub struct AiPrompt {
    input: Entity<InputState>,
    phase: Phase,
    /// Provider key lookup, then the stream. Dropping either cancels the request.
    job: Option<Task<()>>,
    stream: Option<AiStream>,
    reply: String,
    original: String,
    proposal: String,
    diff: Option<AiDiff>,
    /// Why AI can't be used here; replaces the input.
    notice: Option<SharedString>,
    /// What the edit applies to, e.g. "selection" or "whole query".
    scope: Option<SharedString>,
    /// The last instruction sent, for Try again.
    last: String,
    /// Instructions sent from this bar, oldest first.
    history: Vec<String>,
    /// Position while browsing with the arrow keys; `None` when typing fresh text.
    recall: Option<usize>,
    /// Text typed before browsing began, restored by stepping past the newest entry.
    draft: String,
}

impl EventEmitter<AiPromptEvent> for AiPrompt {}

impl AiPrompt {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Describe the change..."));
        cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let text = input.read(cx).value().trim().to_owned();
                if !text.is_empty() && !matches!(this.phase, Phase::Streaming) {
                    cx.emit(AiPromptEvent::Submit(text));
                }
            }
        })
        .detach();
        Self {
            input,
            phase: Phase::Idle,
            job: None,
            stream: None,
            reply: String::new(),
            original: String::new(),
            proposal: String::new(),
            diff: None,
            notice: None,
            scope: None,
            last: String::new(),
            history: Vec::new(),
            recall: None,
            draft: String::new(),
        }
    }

    /// Adds `text` to the recall list, dropping an immediate repeat.
    fn remember(&mut self, text: &str) {
        self.recall = None;
        if self.history.last().map(String::as_str) != Some(text) {
            self.history.push(text.to_owned());
            if self.history.len() > HISTORY_MAX {
                self.history.remove(0);
            }
        }
    }

    /// Up (`older`) or down through earlier instructions. Does nothing while the field holds
    /// text typed fresh, so the arrows still move the caret there.
    fn browse(&mut self, older: bool, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.input.read(cx).value().to_string();
        if self.recall.is_none() && !typed.is_empty() {
            return;
        }
        let next = step_recall(self.history.len(), self.recall, older);
        if self.recall.is_none() && next.is_some() {
            self.draft = typed;
        }
        let text = match next {
            Some(ix) => self.history[ix].clone(),
            None if self.recall.is_some() && !older => take(&mut self.draft),
            None => return,
        };
        self.recall = next;
        self.set_instruction(&text, window, cx);
    }

    pub fn set_scope(&mut self, scope: Option<SharedString>, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            cx.notify();
        }
    }

    pub fn set_notice(&mut self, notice: Option<SharedString>, cx: &mut Context<Self>) {
        self.notice = notice;
        cx.notify();
    }

    pub fn set_instruction(&self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    /// Cancels any request and clears the proposal.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.job = None;
        self.stream = None;
        self.phase = Phase::Idle;
        self.reply.clear();
        self.diff = None;
        cx.notify();
    }

    pub fn start(
        &mut self,
        engine: EngineKind,
        schema: String,
        current: String,
        instruction: String,
        cx: &mut Context<Self>,
    ) {
        self.reset(cx);
        self.remember(&instruction);
        self.last.clone_from(&instruction);
        self.original = current.clone();
        self.phase = Phase::Streaming;
        self.job = Some(launch(
            cx,
            move |model| prompt::edit_request(model, engine, &schema, &current, &instruction),
            |this, stream| this.stream = Some(stream),
            Self::on_event,
        ));
        cx.notify();
    }

    fn on_event(&mut self, event: StreamEvent, cx: &mut Context<Self>) {
        match event {
            StreamEvent::Text(t) => self.reply.push_str(&t),
            StreamEvent::Tool(_) => {}
            StreamEvent::Done(_) => self.finish(cx),
            StreamEvent::Failed(e) => self.fail(e, cx),
        }
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let code = prompt::extract_code(&self.reply);
        if code.trim().is_empty() {
            return self.fail(AiError::protocol("the model returned no query"), cx);
        }
        let code = prompt::match_final_newline(&self.original, code);
        self.diff = AiDiff::new("query.sql", "sql", &self.original, &code, cx);
        self.proposal = code;
        self.phase = Phase::Proposal;
        self.stream = None;
    }

    fn fail(&mut self, error: AiError, cx: &mut Context<Self>) {
        self.stream = None;
        self.phase = Phase::Failed(error);
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let text = take(&mut self.proposal);
        self.reset(cx);
        cx.emit(AiPromptEvent::Apply(text));
    }

    /// Esc: back out one level; close when there is nothing left to cancel.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Idle) {
            cx.emit(AiPromptEvent::Close);
        } else {
            self.reset(cx);
        }
    }

    fn body(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let muted = cx.theme().muted_foreground;
        match &self.phase {
            Phase::Idle => None,
            Phase::Streaming => {
                let preview = prompt::extract_code(&self.reply);
                Some(
                    v_flex()
                        .gap_1()
                        .child(div().text_xs().text_color(muted).child("Generating..."))
                        .when(!preview.is_empty(), |v| {
                            v.child(
                                div()
                                    .id("ai-preview")
                                    .max_h(px(120.0))
                                    .overflow_y_scroll()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_xs()
                                    .child(preview),
                            )
                        })
                        .into_any_element(),
                )
            }
            Phase::Failed(e) => Some(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(error_text(e)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("ai-retry")
                                    .primary()
                                    .small()
                                    .label("Try again")
                                    .disabled(self.last.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let text = this.last.clone();
                                        cx.emit(AiPromptEvent::Submit(text));
                                    })),
                            )
                            .child(
                                Button::new("ai-failed-discard")
                                    .ghost()
                                    .small()
                                    .label("Discard")
                                    .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                            ),
                    )
                    .into_any_element(),
            ),
            Phase::Proposal => {
                let stats = self.diff.as_ref().map(AiDiff::stats);
                Some(
                    v_flex()
                        .gap_2()
                        .child(match (&self.diff, stats) {
                            (Some(diff), Some((add, del))) => v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("+{add} -{del}")),
                                )
                                .child(diff.view(px(220.0)))
                                .into_any_element(),
                            _ => div()
                                .text_sm()
                                .text_color(muted)
                                .child("No changes proposed.")
                                .into_any_element(),
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("ai-apply")
                                        .primary()
                                        .small()
                                        .label("Apply  ⌘⏎")
                                        .disabled(self.diff.is_none())
                                        .on_click(cx.listener(|this, _, _, cx| this.apply(cx))),
                                )
                                .child(
                                    Button::new("ai-discard")
                                        .ghost()
                                        .small()
                                        .label("Discard  Esc")
                                        .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                                ),
                        )
                        .into_any_element(),
                )
            }
        }
    }
}

impl Render for AiPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (border, bg, muted, accent) = (
            theme.border.opacity(0.72),
            theme.muted.opacity(0.18),
            theme.muted_foreground,
            theme.primary,
        );
        let head: gpui_kit::AnyElement = match &self.notice {
            Some(notice) => div()
                .flex_1()
                .text_sm()
                .text_color(muted)
                .child(notice.clone())
                .into_any_element(),
            None => h_flex()
                .flex_1()
                .gap_2()
                .items_center()
                .child(div().flex_1().child(Input::new(&self.input).small()))
                .children(self.scope.clone().map(|scope| {
                    div()
                        .px_2()
                        .rounded_full()
                        .border_1()
                        .border_color(accent.opacity(0.35))
                        .bg(accent.opacity(0.1))
                        .text_xs()
                        .text_color(accent)
                        .child(scope)
                }))
                .into_any_element(),
        };
        let body = if self.notice.is_some() {
            None
        } else {
            self.body(cx)
        };
        v_flex()
            .gap_2()
            .p_2()
            .border_b_1()
            .border_color(border)
            .bg(bg)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "up" || ev.keystroke.key == "down" {
                    this.browse(ev.keystroke.key == "up", window, cx);
                } else if ev.keystroke.key == "escape" {
                    this.dismiss(cx);
                } else if ev.keystroke.key == "enter"
                    && ev.keystroke.modifiers.secondary()
                    && matches!(this.phase, Phase::Proposal)
                    && this.diff.is_some()
                {
                    this.apply(cx);
                }
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::Bot).small().text_color(muted))
                    .child(head)
                    .child(
                        Button::new("ai-close")
                            .ghost()
                            .small()
                            .icon(IconName::Close)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reset(cx);
                                cx.emit(AiPromptEvent::Close);
                            })),
                    ),
            )
            .children(body)
    }
}

#[cfg(test)]
mod tests {
    use super::step_recall;

    #[test]
    fn arrows_walk_the_history() {
        assert_eq!(step_recall(0, None, true), None);
        assert_eq!(step_recall(3, None, true), Some(2));
        assert_eq!(step_recall(3, Some(2), true), Some(1));
        assert_eq!(step_recall(3, Some(0), true), Some(0));
        assert_eq!(step_recall(3, Some(1), false), Some(2));
        assert_eq!(step_recall(3, Some(2), false), None);
        assert_eq!(step_recall(3, None, false), None);
    }
}
