//! Global command shell / omnibar widget for the title bar.

use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, h_flex};

use crate::app::prefs;
use crate::bindings::ToggleCommandPalette;
use crate::widgets::kbd::{kbd, kbd_for_action};
use crate::workspace::WorkspaceRef;

pub fn command_shell(window: &Window, cx: &mut App, placeholder: &'static str) -> impl IntoElement {
    let hint = kbd_for_action(&ToggleCommandPalette, window).unwrap_or_else(|| kbd("cmd-k"));
    h_flex()
        .id("global-command-shell")
        .h(px(28.0))
        .w(px(430.0))
        .max_w(px(520.0))
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(cx.theme().input)
        .bg(cx.theme().tab_active)
        .shadow_xs()
        .hover(|s| s.border_color(cx.theme().border))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                ws.update(cx, |ws, cx| {
                    ws.toggle_command_palette(window, cx);
                });
            }
        })
        .child(
            Icon::new(IconName::Search)
                .text_color(cx.theme().muted_foreground)
                .with_size(prefs::ui_component_size(cx).smaller()),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(placeholder),
        )
        .child(hint)
}
