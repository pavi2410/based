//! Datetime hover card on result-grid cells via gpui-kit HoverCard.

use gpui_kit::component::{ActiveTheme, h_flex, hover_card::HoverCard, v_flex};
use gpui_kit::{
    Anchor, AnyElement, App, ElementId, IntoElement, ParentElement, div, prelude::*, px,
};

use crate::app::prefs;
use crate::widgets::datetime;

/// Wrap a datetime cell in a HoverCard that hangs just under the cell.
pub fn wrap(id: impl Into<ElementId>, cell: impl IntoElement + 'static, raw: &str) -> AnyElement {
    let Some(card) = datetime::hover_card(raw) else {
        return cell.into_any_element();
    };
    let rows = card.rows();
    HoverCard::new(id)
        .anchor(Anchor::TopLeft)
        .trigger(cell)
        .content(move |_, _, cx| card_rows(&rows, cx))
        .into_any_element()
}

fn card_rows(rows: &[(String, String)], cx: &App) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let subtle = cx.theme().muted_foreground;
    let mono = prefs::code_font_family(cx);
    let mut col = v_flex().gap_1().min_w(px(260.0));
    for (label, value) in rows {
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
    col
}
