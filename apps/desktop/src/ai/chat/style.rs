//! Font sizes for the pane, derived from the app's UI and editor sizes.

use gpui_kit::component::{
    ActiveTheme as _,
    text::{TextView, TextViewStyle},
};
use gpui_kit::{App, ElementId, Pixels, SharedString, StyleRefinement, Styled as _, px};

/// Message text: the UI font size.
pub(super) fn body(cx: &App) -> Pixels {
    cx.theme().font_size
}

/// Secondary lines: tool calls, status, timestamps.
pub(super) fn small(cx: &App) -> Pixels {
    cx.theme().font_size - px(1.0)
}

/// Code in replies: a notch under the editor size so blocks don't outshout the text.
pub(super) fn code(cx: &App) -> Pixels {
    cx.theme().mono_font_size - px(1.0)
}

/// A markdown reply sized for the pane.
pub(super) fn markdown(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    cx: &App,
) -> TextView {
    TextView::markdown(id, text)
        .style(TextViewStyle::default().code_block(StyleRefinement::default().text_size(code(cx))))
}
