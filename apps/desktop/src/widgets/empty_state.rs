//! Empty state views — full-panel icon+text and lightweight muted hint variants.

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, h_flex, v_flex};
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

/// Full-panel state with a tone, optional raw detail and action buttons.
///
/// Used where the next step matters: an error with Retry, an empty table, a
/// filter that hides every row.
pub fn status_panel(
    title: &str,
    body: impl Into<SharedString>,
    icon: IconName,
    tone: Option<Hsla>,
    detail: Option<SharedString>,
    actions: Vec<AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let accent = tone.unwrap_or(theme.muted_foreground);
    let title_id = SharedString::from(title.to_string());
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .px(px(24.0))
        .child(
            div()
                .w(px(48.0))
                .h(px(48.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(accent.opacity(if tone.is_some() { 0.45 } else { 0.9 }))
                .bg(accent.opacity(if tone.is_some() { 0.1 } else { 0.12 }))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).text_color(accent).with_animation(
                    SharedString::from(format!("status-pop-{title_id}")),
                    motion::spring_animation(),
                    |icon, t| {
                        let s = motion::pop(t);
                        icon.transform(Transformation::scale(size(s, s)))
                    },
                )),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .max_w(px(420.0))
                        .text_center()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(body.into()),
                ),
        )
        .when_some(detail, |v, detail| {
            v.child(
                div()
                    .max_w(px(480.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(6.0))
                    .bg(theme.muted.opacity(0.4))
                    .text_xs()
                    .font_family(prefs::code_font_family(cx))
                    .text_color(theme.muted_foreground)
                    .child(detail),
            )
        })
        .when(!actions.is_empty(), |v| {
            v.child(h_flex().gap(px(8.0)).children(actions))
        })
        .into_any_element()
}
