//! Empty state views — full-panel icon+text and lightweight muted hint variants.

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, v_flex};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::widgets::motion;

pub fn empty_state(
    title: &'static str,
    body: &'static str,
    icon: IconName,
    cx: &mut App,
) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(48.0))
                .h(px(48.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(cx.theme().border.opacity(0.9))
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(cx.theme().muted.opacity(0.7), 0.0),
                    linear_color_stop(cx.theme().muted.opacity(0.25), 1.0),
                ))
                .shadow_sm()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .text_color(cx.theme().muted_foreground)
                        .with_size(prefs::ui_component_size(cx))
                        .with_animation(
                            SharedString::from(format!("empty-pop-{title}")),
                            motion::spring_animation(),
                            |icon, t| {
                                let s = motion::pop(t);
                                icon.transform(Transformation::scale(size(s, s)))
                            },
                        ),
                )
                .with_animation(
                    SharedString::from(format!("empty-tile-{title}")),
                    Animation::new(motion::RISE),
                    |tile, t| tile.opacity(t.clamp(0.0, 1.0)),
                ),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(4.0))
                .child(motion::rise_in(
                    SharedString::from(format!("empty-title-{title}")),
                    1,
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                ))
                .child(motion::rise_in(
                    SharedString::from(format!("empty-body-{title}")),
                    2,
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                )),
        )
}

/// Lightweight centered muted hint for sidebars and small panes (no icon).
///
/// Use this for inline "nothing to show" states where the full `empty_state`
/// (icon + title + body) would be too heavy.
pub fn pane_empty_hint(message: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .p_3()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(message.into()),
        )
}
