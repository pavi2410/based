//! Section eyebrow — muted bold xs label used as a list section header.

use gpui_kit::component::{ActiveTheme, StyledExt};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;

/// Plain section header: bold xs muted label in a fixed-height row.
///
/// Standard layout: `h_8`, horizontal padding `px_3`. Suitable for sidebar
/// sections that don't need tree-indent alignment.
pub fn section_eyebrow(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    div()
        .h_8()
        .px_3()
        .flex()
        .items_center()
        .text_xs()
        .font_bold()
        .font_family(prefs::ui_font_family(cx))
        .font_weight(prefs::ui_font_weight(cx))
        .text_color(muted.opacity(0.86))
        .child(label.into())
}
