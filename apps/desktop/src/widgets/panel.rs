//! Panel chrome — shell, headers, and toolbar components for boxed content panels.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::app::prefs;
use crate::widgets::layout::{PANEL_RADIUS, panel_header_height};

/// Compact title strip for a boxed panel.
pub fn panel_shell_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    h_flex()
        .h(px(panel_header_height(cx)))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px(px(10.0))
        .border_b_1()
        .border_color(border)
        .bg(cx.theme().muted.opacity(0.22))
        .child(
            v_flex()
                .min_w_0()
                .gap(px(1.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(subtitle.into()),
                ),
        )
}

pub fn panel_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    panel_shell_header(title, subtitle, cx)
}

/// In-panel context line when the dock tab already shows the title.
pub fn panel_context_header(subtitle: impl Into<SharedString>, cx: &mut App) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    h_flex()
        .h(px(panel_header_height(cx)))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px(px(10.0))
        .border_b_1()
        .border_color(border)
        .bg(cx.theme().muted.opacity(0.18))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(subtitle.into()),
        )
}

/// Secondary toolbar row inside a panel shell.
pub fn toolbar_strip(
    cx: &mut App,
    children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .flex_shrink_0()
        .flex_wrap()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.18))
        .children(children)
}

/// Bordered panel frame: optional header + flexible body.
pub fn panel_shell(
    cx: &mut App,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    body: impl IntoElement,
) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    let title = title.into();
    let subtitle = subtitle.into();
    let header: AnyElement = if title.is_empty() {
        panel_context_header(subtitle, cx).into_any_element()
    } else {
        panel_shell_header(title, subtitle, cx).into_any_element()
    };
    v_flex()
        .size_full()
        .border_1()
        .border_color(border)
        .rounded(px(PANEL_RADIUS))
        .overflow_hidden()
        .bg(cx.theme().background)
        .child(header)
        .child(div().flex_1().min_h_0().child(body))
}

pub fn toolbar_button(id: &'static str, icon: IconName, tooltip: &'static str, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .with_size(prefs::ui_component_size(cx).smaller())
        .icon(icon)
        .tooltip(SharedString::from(tooltip))
}
