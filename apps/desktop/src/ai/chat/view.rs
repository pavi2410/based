use based_ai::{Message, Role};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Textarea,
    message_scroller::MessageScroller,
    text::TextView,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Render, StyleRefinement, Styled,
    Window, div, px,
};

use super::{ChatPane, Phase};
use crate::ai::launch::error_text;

impl ChatPane {
    fn row(&self, ix: usize, this: &Entity<Self>, cx: &App) -> AnyElement {
        let Some(turn) = self.turns.get(ix) else {
            return self.live_row(ix, cx);
        };
        match turn.role {
            Role::User => user_row(turn, cx),
            Role::Assistant => assistant_row(turn, ix),
            Role::Tool => self.call_row(ix, this, cx),
        }
    }

    /// The row after the last turn: reply in progress, or why it failed.
    fn live_row(&self, ix: usize, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let body = match &self.phase {
            Phase::Failed(e) => div()
                .text_sm()
                .text_color(theme.danger)
                .child(error_text(e))
                .into_any_element(),
            Phase::Tools => v_flex()
                .gap_1()
                .child(self.running_calls(cx))
                .children(
                    self.pending
                        .is_some()
                        .then(|| muted_line("Waiting for your approval...", cx)),
                )
                .into_any_element(),
            _ if self.draft.is_empty() => muted_line("Thinking...", cx),
            _ => div()
                .text_sm()
                .child(TextView::markdown(("chat-reply", ix), self.draft.clone()).stream_fade(true))
                .into_any_element(),
        };
        div().px_3().py_1p5().child(body).into_any_element()
    }

    fn empty_state(&self, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .px_6()
            .child(Icon::new(IconName::Bot).size_6().text_color(muted))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .text_center()
                    .child("Ask about SQL, Mongo pipelines or Redis commands."),
            )
            .into_any_element()
    }

    /// Active database, plus the notice when assistant access is off.
    fn status_line(&self, cx: &App) -> AnyElement {
        let text = match (self.active(), &self.notice) {
            (Some(c), None) => format!("{} · {}", c.label, c.engine.short_label()),
            (Some(c), Some(n)) => format!("{} · {n}", c.label),
            (None, _) => "No database active".to_owned(),
        };
        div()
            .px_3()
            .pt_1p5()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }

    fn composer(&self, cx: &mut Context<Self>) -> AnyElement {
        let streaming = self.busy();
        let stop = streaming.then(|| {
            Button::new("chat-stop")
                .ghost()
                .small()
                .icon(IconName::Close)
                .tooltip("Stop")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        });
        v_flex()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.72))
            .children(self.approval_card(cx))
            .child(self.status_line(cx))
            .child(
                h_flex()
                    .gap_1()
                    .items_end()
                    .p_2()
                    .child(self.tag_button(cx))
                    .child(div().flex_1().child(Textarea::new(&self.input).small()))
                    .children(stop),
            )
            .into_any_element()
    }
}

impl Render for ChatPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let list: AnyElement = if self.rows() == 0 {
            self.empty_state(cx)
        } else {
            MessageScroller::new("chat-scroll", self.scroller.clone(), move |ix, _, cx| {
                this.read(cx).row(ix, &this, cx)
            })
            // Rows carry their own spacing so consecutive tool calls sit together.
            .with_row_style(StyleRefinement::default().pb_0().px_0())
            .into_any_element()
        };
        v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(list))
            .child(self.composer(cx))
    }
}

fn muted_line(text: &'static str, cx: &App) -> AnyElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

fn user_row(turn: &Message, cx: &App) -> AnyElement {
    h_flex()
        .justify_end()
        .px_3()
        .py_1p5()
        .child(
            div()
                .max_w(px(260.0))
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(cx.theme().primary.opacity(0.14))
                .text_sm()
                .child(turn.content.clone()),
        )
        .into_any_element()
}

fn assistant_row(turn: &Message, ix: usize) -> AnyElement {
    if turn.content.is_empty() {
        // Tool-call-only turn: the calls render as their own rows.
        return div().h(px(0.0)).into_any_element();
    }
    div()
        .px_3()
        .py_1p5()
        .text_sm()
        .child(TextView::markdown(("chat-reply", ix), turn.content.clone()).selectable(true))
        .into_any_element()
}
