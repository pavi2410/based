//! Transcript rows: one per user message, and one per run of assistant turns (text and
//! tool calls) laid out with [`MessageGroup`].

use std::mem::take;
use std::ops::Range;

use based_ai::{Message, Role};
use gpui_kit::component::{message::MessageGroup, v_flex};
use gpui_kit::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, div, px};

use super::{ChatPane, Phase, style};
use crate::ai::launch::error_text;
use gpui_kit::component::ActiveTheme;

/// Turn ranges that share a row: each user message alone, then consecutive assistant and
/// tool-result turns together.
pub(super) fn turn_groups(turns: &[Message]) -> Vec<Range<usize>> {
    let mut groups: Vec<Range<usize>> = Vec::new();
    for (ix, turn) in turns.iter().enumerate() {
        match groups.last_mut() {
            Some(last) if turn.role != Role::User && turns[last.start].role != Role::User => {
                last.end = ix + 1;
            }
            _ => groups.push(ix..ix + 1),
        }
    }
    groups
}

impl ChatPane {
    /// The reply being generated, a failure, or the tools running.
    fn has_live(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// The live content continues the last assistant group instead of getting its own row.
    fn live_joins_last(&self) -> bool {
        self.has_live() && self.turns.last().is_some_and(|t| t.role != Role::User)
    }

    pub(super) fn rows(&self) -> usize {
        let groups = turn_groups(&self.turns).len();
        groups + usize::from(self.has_live() && !self.live_joins_last())
    }

    pub(super) fn row(&self, ix: usize, this: &Entity<Self>, cx: &App) -> AnyElement {
        let groups = turn_groups(&self.turns);
        let Some(range) = groups.get(ix).cloned() else {
            return div().px_3().child(self.live(cx)).into_any_element();
        };
        let first = &self.turns[range.start];
        if first.role == Role::User {
            return super::view::user_row(first, cx);
        }
        let live = ix + 1 == groups.len() && self.live_joins_last();
        self.assistant_group(range, live, this, cx)
    }

    fn assistant_group(
        &self,
        range: Range<usize>,
        live: bool,
        this: &Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let mut group = MessageGroup::new().px_3();
        let mut calls: Vec<AnyElement> = Vec::new();
        for ix in range {
            let turn = &self.turns[ix];
            if turn.role == Role::Tool {
                calls.push(self.call_row(ix, this, cx));
                continue;
            }
            group = flush(group, &mut calls);
            if !turn.content.is_empty() {
                group = group.child(div().text_size(style::body(cx)).child(
                    style::markdown(("chat-reply", ix), turn.content.clone(), cx).selectable(true),
                ));
            }
        }
        group = flush(group, &mut calls);
        if live {
            group = group.child(self.live(cx));
        }
        group.into_any_element()
    }

    /// Streaming text, running tools, the error, or a "Thinking..." placeholder.
    fn live(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        match &self.phase {
            Phase::Failed(e) => div()
                .text_size(style::body(cx))
                .text_color(theme.danger)
                .child(error_text(e))
                .into_any_element(),
            Phase::Tools => v_flex()
                .gap_1()
                .child(self.running_calls(cx))
                .children(
                    self.pending
                        .is_some()
                        .then(|| super::view::muted_line("Waiting for your approval...", cx)),
                )
                .into_any_element(),
            _ if self.draft.is_empty() => super::view::muted_line("Thinking...", cx),
            _ => div()
                .text_size(style::body(cx))
                .child(
                    style::markdown(("chat-reply", self.turns.len()), self.draft.clone(), cx)
                        .stream_fade(true),
                )
                .into_any_element(),
        }
    }

    /// Row holding turn `ix`.
    pub(super) fn row_of_turn(&self, ix: usize) -> Option<usize> {
        turn_groups(&self.turns)
            .iter()
            .position(|g| g.contains(&ix))
    }
}

/// Adds pending tool-call lines as one tight block.
fn flush(group: MessageGroup, calls: &mut Vec<AnyElement>) -> MessageGroup {
    if calls.is_empty() {
        return group;
    }
    group.child(v_flex().gap(px(2.0)).children(take(calls)))
}

#[cfg(test)]
mod tests {
    use based_ai::tools::{ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    #[test]
    fn groups_split_at_user_messages() {
        let call = ToolCall {
            id: "1".into(),
            name: "list_tables".into(),
            args: json!({}),
        };
        let turns = vec![
            Message::user("a"),
            Message::assistant_calls("", vec![call.clone()]),
            Message::tool(&call, ToolResult::ok("t")),
            Message::assistant("done"),
            Message::user("b"),
            Message::assistant("ok"),
        ];
        assert_eq!(turn_groups(&turns), [0..1, 1..4, 4..5, 5..6]);
        assert!(turn_groups(&[]).is_empty());
    }
}
