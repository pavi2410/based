use std::collections::HashMap;

use based_ai::tools::ToolCall;
use based_ai::{Message, Role};
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
use crate::app::prefs;

impl ChatPane {
    fn row(&self, ix: usize, cx: &App) -> AnyElement {
        let Some(turn) = self.turns.get(ix) else {
            return self.live_row(ix, cx);
        };
        match turn.role {
            Role::User => user_row(turn, cx),
            Role::Assistant => assistant_row(turn, ix, &self.targets, cx),
            Role::Tool => tool_row(turn, cx),
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
            Phase::Tools => muted_line("Running tools...", cx),
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

    fn tag_chips(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap_1()
            .px_2()
            .children(self.tags.iter().enumerate().map(|(ix, tag)| {
                Button::new(("chat-untag", ix))
                    .ghost()
                    .small()
                    .label(format!("@{tag}"))
                    .icon(IconName::Close)
                    .tooltip("Remove tag")
                    .on_click(cx.listener(move |this, _, _, cx| this.remove_tag(ix, cx)))
            }))
            .into_any_element()
    }

    fn composer(&self, cx: &mut Context<Self>) -> AnyElement {
        let streaming = self.busy();
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
        v_flex()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.72))
            .child(self.status_line(cx))
            .child(self.tag_chips(cx))
            .child(
                h_flex()
                    .gap_1()
                    .items_end()
                    .p_2()
                    .child(self.tag_button(cx))
                    .child(div().flex_1().child(Textarea::new(&self.input).small()))
                    .child(action),
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

fn assistant_row(
    turn: &Message,
    ix: usize,
    targets: &HashMap<String, String>,
    cx: &App,
) -> AnyElement {
    let mut col = v_flex().px_3().py_1p5().gap_1().text_sm();
    if !turn.content.is_empty() {
        col = col
            .child(TextView::markdown(("chat-reply", ix), turn.content.clone()).selectable(true));
    }
    for call in &turn.calls {
        col = col.child(
            div()
                .text_xs()
                .font_family(prefs::code_font_family(cx))
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "> {}{}",
                    call_summary(call),
                    targets
                        .get(&call.id)
                        .map_or_else(String::new, |t| format!("  @ {t}"))
                )),
        );
    }
    col.into_any_element()
}

fn tool_row(turn: &Message, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let failed = turn.outcome.as_ref().is_some_and(|o| o.is_error);
    let color = if failed {
        theme.danger
    } else {
        theme.muted_foreground
    };
    div()
        .mx_3()
        .my_0p5()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(theme.muted.opacity(0.25))
        .text_xs()
        .font_family(prefs::code_font_family(cx))
        .text_color(color)
        .line_clamp(3)
        .child(turn.content.clone())
        .into_any_element()
}

/// `name arg1 arg2`, clipped to one short line.
fn call_summary(call: &ToolCall) -> String {
    const MAX: usize = 80;
    let args = call.args.as_object().into_iter().flat_map(|o| o.values());
    let mut out = call.name.clone();
    for v in args {
        out.push(' ');
        match v.as_str() {
            Some(s) => out.push_str(s),
            None => out.push_str(&v.to_string()),
        }
    }
    let out = out.replace('\n', " ");
    if out.chars().count() > MAX {
        format!("{}...", out.chars().take(MAX).collect::<String>())
    } else {
        out
    }
}
