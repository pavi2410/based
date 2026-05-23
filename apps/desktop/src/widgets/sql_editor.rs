//! SQL/JSON editors via gpui-component `InputState::code_editor` (tree-sitter highlighting).

use gpui::{App, Entity, Hsla, IntoElement, ParentElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputState},
};

pub fn new_code_input(
    language: &'static str,
    initial: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .code_editor(language)
            .line_number(true)
            .searchable(true)
    });
    input.update(cx, |state, cx| {
        state.set_value(initial, window, cx);
    });
    input
}

pub fn new_sql_input(initial: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    new_code_input("sql", initial, window, cx)
}

pub fn new_json_input(initial: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    new_code_input("json", initial, window, cx)
}

pub fn text_from_input(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

pub fn set_input_text(
    input: &Entity<InputState>,
    text: &str,
    window: &mut Window,
    cx: &mut App,
) {
    input.update(cx, |state, cx| {
        state.set_value(text, window, cx);
    });
}

/// Back-compat aliases for SQL query panels.
pub fn sql_from_input(input: &Entity<InputState>, cx: &App) -> String {
    text_from_input(input, cx)
}

pub fn set_sql_input(
    input: &Entity<InputState>,
    sql: &str,
    window: &mut Window,
    cx: &mut App,
) {
    set_input_text(input, sql, window, cx);
}

/// Full-height code editor surface for query / pipeline panels.
pub fn code_editor_area(
    input: &Entity<InputState>,
    is_error: bool,
    height: f32,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let border: Hsla = if is_error {
        theme.danger
    } else {
        theme.border
    };

    div()
        .h(px(height))
        .p_2()
        .border_1()
        .rounded(px(7.0))
        .bg(theme.muted.opacity(0.14))
        .border_color(border)
        .child(Input::new(input).h_full().cleanable(false))
}
