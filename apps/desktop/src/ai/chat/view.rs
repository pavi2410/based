use based_ai::Message;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Textarea,
    message_scroller::MessageScroller,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, StyleRefinement, Styled, Window,
    div, px,
};

use super::{ChatPane, style};

impl ChatPane {
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
                    .text_size(style::body(cx))
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
            .text_size(style::small(cx))
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
            .with_row_style(StyleRefinement::default().pb(px(12.0)).px_0())
            .into_any_element()
        };
        let sticky = self.sticky_user(cx);
        v_flex()
            .size_full()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(list)
                    .children(sticky.map(|s| div().absolute().top_0().left_0().right_0().child(s))),
            )
            .child(self.composer(cx))
    }
}

pub(super) fn muted_line(text: &'static str, cx: &App) -> AnyElement {
    div()
        .text_size(style::body(cx))
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

/// The user's message as a full-width filled block; `lines` clips it (pinned copy).
pub(super) fn user_row(turn: &Message, lines: Option<usize>, cx: &App) -> AnyElement {
    let mut text = div().child(turn.content.clone());
    if let Some(lines) = lines {
        text = text.line_clamp(lines);
    }
    let theme = cx.theme();
    let tint = theme.primary.opacity(0.14);
    // The pinned copy floats over the list, so its fill must be opaque.
    let fill = if lines.is_some() {
        theme.background.blend(tint)
    } else {
        tint
    };
    div()
        .px_3()
        .child(
            div()
                .w_full()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(fill)
                .text_size(style::body(cx))
                .child(text),
        )
        .into_any_element()
}
