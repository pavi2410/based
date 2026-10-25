use based_ai::Role;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Textarea,
    message_scroller::MessageScroller,
    text::TextView,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, Styled, Window, div, px,
};

use super::{ChatPane, Phase};
use crate::ai::launch::error_text;

impl ChatPane {
    fn row(&self, ix: usize, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let Some(turn) = self.turns.get(ix) else {
            return self.live_row(ix, cx);
        };
        match turn.role {
            Role::User => h_flex()
                .justify_end()
                .px_3()
                .py_1p5()
                .child(
                    div()
                        .max_w(px(260.0))
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(theme.primary.opacity(0.14))
                        .text_sm()
                        .child(turn.content.clone()),
                )
                .into_any_element(),
            Role::Tool => div().into_any_element(),
            Role::Assistant => div()
                .px_3()
                .py_1p5()
                .text_sm()
                .child(
                    TextView::markdown(("chat-reply", ix), turn.content.clone()).selectable(true),
                )
                .into_any_element(),
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
            _ if self.draft.is_empty() => div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Thinking...")
                .into_any_element(),
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

    fn composer(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(notice) = &self.notice {
            return div()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(notice.clone())
                .into_any_element();
        }
        let streaming = self.streaming();
        let action = if streaming {
            Button::new("chat-stop")
                .ghost()
                .small()
                .icon(IconName::Close)
                .tooltip("Stop")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("chat-clear")
                .ghost()
                .small()
                .icon(IconName::Delete)
                .tooltip("Clear conversation")
                .disabled(self.turns.is_empty())
                .on_click(cx.listener(|this, _, _, cx| this.clear(cx)))
        };
        h_flex()
            .gap_1()
            .items_end()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.72))
            .child(div().flex_1().child(Textarea::new(&self.input).small()))
            .child(action)
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
                this.read(cx).row(ix, cx)
            })
            .into_any_element()
        };
        v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(list))
            .child(self.composer(cx))
    }
}
