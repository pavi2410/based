//! Shared labeled inputs for connection wizards.

use gpui_kit::component::{ActiveTheme, h_flex, input::InputState, v_flex};
use gpui_kit::{
    App, Context, Entity, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*,
};

pub fn new_field<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    default: &str,
    placeholder: &str,
) -> Entity<InputState> {
    let default = default.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(default)
    })
}

pub fn set_field(input: &Entity<InputState>, value: &str, window: &mut Window, cx: &mut App) {
    input.update(cx, |state, cx| {
        state.set_value(value, window, cx);
    });
}

pub fn labeled_field(title: &str, muted: Hsla, input: impl IntoElement) -> impl IntoElement {
    labeled_field_inner(title, muted, input).flex_1()
}

pub fn labeled_fixed(
    title: &str,
    muted: Hsla,
    width: f32,
    input: impl IntoElement,
) -> impl IntoElement {
    labeled_field_inner(title, muted, input).w(gpui_kit::px(width))
}

fn labeled_field_inner(title: &str, muted: Hsla, input: impl IntoElement) -> gpui_kit::Div {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(SharedString::from(title.to_string())),
        )
        .child(div().w_full().child(input))
}

/// Form section header: small caps label with a hairline running to the edge.
pub fn form_section(title: &'static str, cx: &App) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .gap_2()
        .pt_2()
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title.to_uppercase()),
        )
        .child(div().flex_1().h(gpui_kit::px(1.0)).bg(cx.theme().border))
}
