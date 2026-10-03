//! Read-only datetime card placed beside the hovered grid cell.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::base::{Align, ElementExt, Positioner};
use gpui_kit::component::{ActiveTheme, Placement, ThemeStyled, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, IntoElement, ParentElement, RenderOnce, Styled, Window,
    deferred, div, prelude::*, px,
};

use crate::app::prefs;
use crate::widgets::datetime;

struct HoverOpen {
    open: bool,
}

/// Wrap a datetime cell so its hover card sits to the right, not over the next row.
pub fn wrap(id: impl Into<ElementId>, cell: impl IntoElement, raw: &str) -> AnyElement {
    let Some(card) = datetime::hover_card(raw) else {
        return cell.into_any_element();
    };
    BesideCard {
        id: id.into(),
        trigger: cell.into_any_element(),
        rows: card.rows(),
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct BesideCard {
    id: ElementId,
    trigger: AnyElement,
    rows: [(String, String); 4],
}

impl RenderOnce for BesideCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| HoverOpen { open: false });
        let open = state.read(cx).open;
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let capture = bounds.clone();

        let trigger = div()
            .id("dt-cell")
            .w_full()
            .on_prepaint(move |b, _, _| capture.set(b))
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                if state.open != *hovered {
                    state.open = *hovered;
                    cx.notify();
                }
            }))
            .child(self.trigger);

        div()
            .id(self.id)
            .w_full()
            .child(trigger)
            .when(open, |this| {
                this.child(
                    deferred(
                        Positioner::side(bounds.get())
                            .placement(Placement::Right)
                            .align(Align::Start)
                            .offset(px(8.))
                            .margin(px(8.))
                            .child(card_surface(&self.rows, cx)),
                    )
                    .with_priority(100),
                )
            })
    }
}

fn card_surface(rows: &[(String, String)], cx: &App) -> impl IntoElement {
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
    v_flex().popover_style(cx).p_3().child(col)
}
