//! Asks the user before the assistant opens a connection or runs a query.

use std::collections::HashSet;

use based_ai::tools::{CONNECT, RUN_QUERY, ToolCall};
use futures::channel::oneshot;
use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div};

use super::{ChatPane, style};
use crate::app::prefs;
use crate::connection::ConnectionState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    Deny,
    Once,
    /// Allow this kind of action for the rest of the conversation.
    Always,
}

/// Kinds of action that need approval; `Always` trusts a whole kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Action {
    Connect,
    Query,
}

/// What the user is being asked about.
pub(super) struct Approval {
    kind: Action,
    title: String,
    detail: String,
    reply: oneshot::Sender<Verdict>,
}

/// Kinds the user allowed for the current conversation.
pub(super) type Trusted = HashSet<Action>;

impl ChatPane {
    /// `Some` when `call` must wait for the user; the answer arrives on the receiver.
    pub(super) fn gate(
        &mut self,
        call: &ToolCall,
        cx: &mut Context<Self>,
    ) -> Option<oneshot::Receiver<Verdict>> {
        let (kind, title, detail) = match call.name.as_str() {
            CONNECT => {
                let name = call.args["connection"].as_str()?;
                let entry = self.find(name, cx)?;
                if matches!(entry.read(cx).state, ConnectionState::Connected(_)) {
                    return None;
                }
                (
                    Action::Connect,
                    format!("Connect to {name}?"),
                    String::new(),
                )
            }
            RUN_QUERY => {
                let label = self.active().map_or("the database", |c| c.label.as_str());
                let query = call.args["query"].as_str().unwrap_or_default();
                (
                    Action::Query,
                    format!("Run this query on {label}?"),
                    query.to_owned(),
                )
            }
            _ => return None,
        };
        if self.trusted.contains(&kind) {
            return None;
        }
        let (reply, rx) = oneshot::channel();
        self.pending = Some(Approval {
            kind,
            title,
            detail,
            reply,
        });
        cx.notify();
        Some(rx)
    }

    fn answer(&mut self, verdict: Verdict, cx: &mut Context<Self>) {
        if let Some(a) = self.pending.take() {
            if verdict == Verdict::Always {
                self.trusted.insert(a.kind);
            }
            let _ = a.reply.send(verdict);
        }
        cx.notify();
    }

    /// The question and its buttons, shown above the composer.
    pub(super) fn approval_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let a = self.pending.as_ref()?;
        let (body, small, code) = (
            style::body(cx),
            style::small(cx),
            prefs::code_font_family(cx),
        );
        let theme = cx.theme();
        let mut card = v_flex()
            .gap_1p5()
            .m_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme.warning)
            .bg(theme.muted.opacity(0.25))
            .child(
                div()
                    .text_size(body)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(a.title.clone()),
            );
        if !a.detail.is_empty() {
            card = card.child(
                div()
                    .text_size(small)
                    .font_family(code)
                    .line_clamp(8)
                    .child(a.detail.clone()),
            );
        }
        Some(
            card.child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("chat-allow")
                            .primary()
                            .small()
                            .label("Allow")
                            .on_click(cx.listener(|t, _, _, cx| t.answer(Verdict::Once, cx))),
                    )
                    .child(
                        Button::new("chat-allow-chat")
                            .ghost()
                            .small()
                            .label("Allow for this chat")
                            .on_click(cx.listener(|t, _, _, cx| t.answer(Verdict::Always, cx))),
                    )
                    .child(
                        Button::new("chat-deny")
                            .ghost()
                            .small()
                            .label("Deny")
                            .on_click(cx.listener(|t, _, _, cx| t.answer(Verdict::Deny, cx))),
                    ),
            )
            .into_any_element(),
        )
    }
}
