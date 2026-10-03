//! Read-only datetime card placed beside the hovered grid cell.

use std::cell::Cell;

use gpui_kit::base::{Align, ElementExt, Positioner};
use gpui_kit::component::{ActiveTheme, Placement, ThemeStyled, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, IntoElement, ParentElement, Pixels, RenderOnce, Styled,
    Window, deferred, div, prelude::*, px,
};

use crate::app::prefs;
use crate::widgets::datetime;

struct HoverOpen {
    open: bool,
    bounds: Cell<Bounds<Pixels>>,
    captured: Cell<bool>,
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
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| HoverOpen {
            open: false,
            bounds: Cell::new(Bounds::default()),
            captured: Cell::new(false),
        });
        let open = state.read(cx).open;
        let trigger_bounds = state.read(cx).bounds.get();
        let captured = state.read(cx).captured.get() && usable_bounds(trigger_bounds);

        let trigger = div()
            .id((self.id.clone(), "trigger"))
            .w_full()
            .h_full()
            .on_prepaint({
                let state = state.clone();
                move |bounds, window, cx| {
                    let first = !state.read(cx).captured.get();
                    state.read(cx).bounds.set(bounds);
                    state.read(cx).captured.set(true);
                    if first {
                        window.request_animation_frame();
                    }
                }
            })
            .on_hover(window.listener_for(&state, |state, hovered, window, cx| {
                if state.open != *hovered {
                    state.open = *hovered;
                    cx.notify();
                    if *hovered && !usable_bounds(state.bounds.get()) {
                        window.request_animation_frame();
                    }
                }
            }))
            .child(self.trigger);

        div()
            .id(self.id)
            .w_full()
            .h_full()
            .child(trigger)
            .when(open && captured, |this| {
                this.child(
                    deferred(
                        Positioner::side(trigger_bounds)
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

fn usable_bounds(bounds: Bounds<Pixels>) -> bool {
    bounds.size.width > px(0.) && bounds.size.height > px(0.)
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
