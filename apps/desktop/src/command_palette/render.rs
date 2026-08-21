use gpui::{Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px};
use gpui_component::v_flex;

use super::CommandPalette;

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let palette = cx.weak_entity();
        let query_owner = palette.clone();
        let confirm_owner = palette.clone();
        let cancel_owner = palette.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dismiss(cx);
                }),
            )
            .child(
                v_flex()
                    .absolute()
                    .top(px(120.0))
                    .left_1_2()
                    .ml(px(-280.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(super::entries::command_for_sections(
                        &self.command_state,
                        &self.sections,
                        move |query, _, cx| {
                            _ = query_owner.update(cx, |this, cx| this.on_query(query, cx));
                        },
                        move |index, _, cx| {
                            _ = confirm_owner.update(cx, |this, cx| this.confirm(index, cx));
                        },
                        move |_, cx| {
                            _ = cancel_owner.update(cx, |this, cx| this.dismiss(cx));
                        },
                    )),
            )
            .into_any_element()
    }
}
