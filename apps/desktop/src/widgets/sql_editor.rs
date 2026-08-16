//! SQL/JSON editors via gpui-component `InputState::code_editor` (tree-sitter highlighting).

use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Entity, Hsla, IntoElement, ParentElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme,
    input::{CompletionProvider, Editor, EditorState},
};

use crate::app::prefs;
use crate::editor::{SchemaCache, sql_completion::SqlCompletionProvider};

pub fn new_code_input(
    language: &'static str,
    initial: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<EditorState> {
    let input = cx.new(|cx| {
        EditorState::new(window, cx)
            .language(language)
            .line_number(true)
            .searchable(true)
    });
    input.update(cx, |state, cx| {
        state.set_value(initial, window, cx);
    });
    input
}

pub fn new_sql_input(initial: &str, window: &mut Window, cx: &mut App) -> Entity<EditorState> {
    new_code_input("sql", initial, window, cx)
}

/// Attach schema-aware SQL completions to an existing SQL input.
pub fn attach_sql_completion(
    input: &Entity<EditorState>,
    schema_cache: Arc<SchemaCache>,
    cx: &mut App,
) {
    let provider: Rc<dyn CompletionProvider> = SqlCompletionProvider::new(schema_cache);
    input.update(cx, |state, _| {
        state.lsp_mut().completion_provider = Some(provider);
    });
}

pub fn new_json_input(initial: &str, window: &mut Window, cx: &mut App) -> Entity<EditorState> {
    new_code_input("json", initial, window, cx)
}

pub fn text_from_input(input: &Entity<EditorState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

pub fn set_input_text(input: &Entity<EditorState>, text: &str, window: &mut Window, cx: &mut App) {
    input.update(cx, |state, cx| {
        state.set_value(text, window, cx);
    });
}

fn code_editor_shell(
    input: &Entity<EditorState>,
    is_error: bool,
    readonly: bool,
    cx: &App,
    height: Option<f32>,
) -> impl IntoElement {
    let theme = cx.theme();
    let border: Hsla = if is_error { theme.danger } else { theme.border };
    let font = prefs::code_font_family(cx);
    let weight = prefs::code_font_weight(cx);
    let size = px(prefs::editor_size_token(cx).editor_px());

    let mut shell = div()
        .p_2()
        .border_1()
        .rounded(px(7.0))
        .border_color(border)
        .font_family(font)
        .font_weight(weight)
        .text_size(size);

    if let Some(height) = height {
        shell = shell.h(px(height));
    } else {
        shell = shell.flex_1().min_h_0().h_full();
    }

    shell.child(Editor::new(input).h_full().readonly(readonly))
}

/// Full-height code editor surface for query / pipeline panels.
pub fn code_editor_area(
    input: &Entity<EditorState>,
    is_error: bool,
    height: f32,
    cx: &App,
) -> impl IntoElement {
    code_editor_shell(input, is_error, false, cx, Some(height))
}

/// Flex child that fills remaining panel height (schema DDL, query editors).
pub fn code_editor_flex(
    input: &Entity<EditorState>,
    is_error: bool,
    readonly: bool,
    cx: &App,
) -> impl IntoElement {
    code_editor_shell(input, is_error, readonly, cx, None)
}
