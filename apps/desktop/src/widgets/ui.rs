//! Shared graphite-native UI primitives for the workspace.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    description_list::{DescriptionItem, DescriptionList},
    h_flex, v_flex,
};

use crate::connection::EngineKind;

/// Boxy panel corner radius (Linear / Vercel–style).
pub const PANEL_RADIUS: f32 = 4.0;
pub const PANEL_HEADER_H: f32 = 32.0;
/// Horizontal inset for sidebar list rows and section headers.
pub const SIDEBAR_INSET: f32 = 8.0;

pub fn engine_label(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "PG",
        EngineKind::MongoDB => "MDB",
        EngineKind::SQLite => "SQL",
    }
}

pub fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "Postgres",
        EngineKind::MongoDB => "MongoDB",
        EngineKind::SQLite => "SQLite",
    }
}

pub fn engine_color(engine: EngineKind) -> Hsla {
    match engine {
        EngineKind::Postgres => hsla(0.58, 0.78, 0.64, 1.0),
        EngineKind::MongoDB => hsla(0.38, 0.62, 0.54, 1.0),
        EngineKind::SQLite => hsla(0.10, 0.74, 0.59, 1.0),
    }
}

/// Compact engine label for sidebar rows (no border or fill).
pub fn engine_label_inline(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .font_family(cx.theme().mono_font_family.clone())
        .text_color(color)
        .child(engine_label(engine))
}

pub fn engine_chip(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    h_flex()
        .h(px(18.0))
        .px(px(6.0))
        .gap(px(4.0))
        .items_center()
        .rounded(px(4.0))
        .bg(color.opacity(0.13))
        .border_1()
        .border_color(color.opacity(0.18))
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(color.opacity(0.9)),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(color)
                .child(engine_label(engine)),
        )
}

/// Muted inline hint for window chrome (no border or fill).
pub fn chrome_hint(text: impl Into<SharedString>, cx: &mut App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground.opacity(0.9))
        .child(text.into())
}

/// Horizontal key-value list for narrow panels (e.g. the right inspector).
pub fn compact_description_list_horizontal(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    DescriptionList::horizontal()
        .small()
        .bordered(false)
        .label_width(px(88.0))
        .children(items)
}

/// Vertical key-value list for dashboard cards and schema stats.
pub fn compact_description_list_vertical(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    bordered: bool,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    DescriptionList::vertical()
        .small()
        .bordered(bordered)
        .children(items)
}

/// Inspector sidebar section: mono title + compact horizontal description list.
pub fn inspector_description_section(
    title: &'static str,
    rows: impl IntoIterator<Item = (&'static str, impl Into<SharedString>)>,
    cx: &mut App,
) -> impl IntoElement {
    let items: Vec<(SharedString, SharedString)> = rows
        .into_iter()
        .map(|(label, value)| (label.into(), value.into()))
        .collect();
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(compact_description_list_horizontal(items))
}

pub fn metadata_pill(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    h_flex()
        .h(px(20.0))
        .items_center()
        .gap(px(5.0))
        .px(px(7.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.34))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().foreground.opacity(0.9))
                .child(value.into()),
        )
}

pub fn command_shell(cx: &mut App, placeholder: &'static str) -> impl IntoElement {
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
        .border_color(cx.theme().border.opacity(0.82))
        .bg(cx.theme().muted.opacity(0.44))
        .hover(|s| s.border_color(hsla(0.68, 0.45, 0.68, 0.58)))
        .child(
            Icon::new(IconName::Search)
                .text_color(cx.theme().muted_foreground)
                .with_size(gpui_component::Size::XSmall),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(placeholder),
        )
        .child(
            div()
                .text_xs()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground.opacity(0.82))
                .child(if cfg!(target_os = "macos") {
                    "⌘K"
                } else {
                    "Ctrl K"
                }),
        )
}

/// Compact title strip for a boxed panel.
pub fn panel_shell_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    h_flex()
        .h(px(PANEL_HEADER_H))
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
        .h(px(PANEL_HEADER_H))
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

pub fn toolbar_button(id: &'static str, icon: IconName, tooltip: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .tooltip(SharedString::from(tooltip))
}

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
                .w(px(42.0))
                .h(px(42.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(cx.theme().border.opacity(0.82))
                .bg(cx.theme().muted.opacity(0.4))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .text_color(cx.theme().muted_foreground)
                        .with_size(gpui_component::Size::Medium),
                ),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(3.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                ),
        )
}
