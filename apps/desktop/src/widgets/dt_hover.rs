//! Datetime hover card on result-grid cells via gpui-kit HoverCard.

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, div, prelude::*, px};

use crate::app::prefs;
use crate::widgets::datetime;

/// Card body for a datetime cell; `None` when `raw` is not an instant.
pub fn body(raw: &str, cx: &App) -> Option<AnyElement> {
    let rows = datetime::hover_card(raw)?.rows();
    let fg = cx.theme().foreground;
    let subtle = cx.theme().muted_foreground;
    let mono = prefs::code_font_family(cx);
    let mut col = v_flex().gap_1().min_w(px(260.0));
    for (label, value) in &rows {
        col = col.child(
            h_flex()
                .gap_6()
                .justify_between()
                .child(div().text_xs().text_color(subtle).child(label.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(fg)
                        .font_family(mono.clone())
                        .child(value.clone()),
                ),
        );
    }
    Some(col.into_any_element())
}
