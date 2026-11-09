//! A failed request in the transcript: what went wrong, and the button that fixes it.

use based_ai::{AiError, AiErrorKind};
use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, div};

use super::{ChatPane, Phase, style};
use crate::ai::launch::error_text;
use crate::app::shell::open_settings;

/// Short heading for each kind of failure.
fn heading(kind: AiErrorKind) -> &'static str {
    match kind {
        AiErrorKind::Auth => "The provider rejected the key",
        AiErrorKind::RateLimited => "The provider asked you to slow down",
        AiErrorKind::Network => "Could not reach the provider",
        AiErrorKind::BadRequest => "The provider rejected the request",
        AiErrorKind::Protocol | AiErrorKind::Other => "The reply did not arrive",
    }
}

impl ChatPane {
    pub(super) fn failure(&self, e: &AiError, this: &Entity<Self>, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let danger = theme.danger;
        let retry = this.clone();
        let mut actions = h_flex().gap_1().child(
            Button::new("chat-retry")
                .primary()
                .small()
                .label("Retry")
                .on_click(move |_, _, cx| {
                    retry.update(cx, |p, cx| p.retry(cx));
                }),
        );
        if e.kind == AiErrorKind::Auth {
            actions = actions.child(
                Button::new("chat-open-settings")
                    .small()
                    .label("Open AI settings")
                    .on_click(|_, _, cx| open_settings(cx)),
            );
        }
        v_flex()
            .gap_1p5()
            .child(
                div()
                    .text_size(style::body(cx))
                    .text_color(danger)
                    .child(heading(e.kind)),
            )
            .child(
                div()
                    .text_size(style::small(cx))
                    .text_color(theme.muted_foreground)
                    .child(error_text(e)),
            )
            .child(actions)
            .into_any_element()
    }

    /// Asks again after a failure; the conversation is exactly as it was.
    pub(super) fn retry(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Failed(_)) {
            self.begin_round(cx);
        }
    }
}
