//! Shared labeled inputs for connection wizards.

use gpui::{
    App, Context, Entity, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*,
};
use gpui_component::{input::InputState, v_flex};

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
    v_flex()
        .flex_1()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(SharedString::from(title.to_string())),
        )
        .child(div().w_full().child(input))
}
