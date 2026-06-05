//! Panel chrome — shell, headers, and toolbar components for boxed content panels.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::app::prefs;
use crate::connection::{ConnectionId, EngineKind};
use crate::project::RegistryRef;
use crate::widgets::engine_icon;
use crate::widgets::layout::{PANEL_RADIUS, panel_header_height};
use crate::widgets::metadata_pill;
use crate::widgets::status_item::STATUS_BAR_HEIGHT;

/// Bottom-of-tab breadcrumb rail height (matches status bar segment height).
pub const TAB_BREADCRUMB_HEIGHT: f32 = STATUS_BAR_HEIGHT;

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

/// Active/inactive styling for a tab-switcher button inside an inspector panel.
///
/// Attach `.on_click(cx.listener(...))` to the returned `Button` to wire up the
/// selection logic — the `on_click` must stay at the call site because it
/// captures `cx.listener`, which binds to the specific panel `Context`.
pub fn tab_button_styled(id: &'static str, label: &'static str, active: bool) -> Button {
    let b = Button::new(id).label(label).small();
    if active { b.outline() } else { b.ghost() }
}

#[derive(Clone, Debug)]
pub struct TabBreadcrumb {
    pub engine: EngineKind,
    pub segments: Vec<SharedString>,
}

/// Build breadcrumb segments: connection label from registry, then `tail` (schema, table, …).
pub fn tab_breadcrumb_for_connection(
    conn_id: &ConnectionId,
    tail: impl IntoIterator<Item = impl Into<SharedString>>,
    cx: &App,
) -> TabBreadcrumb {
    let (engine, conn_label) = cx
        .try_global::<RegistryRef>()
        .and_then(|r| r.0.read(cx).get(conn_id, cx))
        .map(|entry| {
            let entry = entry.read(cx);
            (entry.config.engine(), entry.config.label().to_string())
        })
        .unwrap_or_else(|| (EngineKind::Postgres, conn_id.0.clone()));

    let mut segments = vec![conn_label.into()];
    segments.extend(tail.into_iter().map(Into::into));
    TabBreadcrumb { engine, segments }
}

/// Trailing cluster for data-viewer breadcrumbs: row range, load time, read-only indicator.
pub fn tab_breadcrumb_data_viewer_trailing(
    rows_value: impl Into<SharedString>,
    load_ms: Option<u64>,
    read_only_id: &'static str,
    cx: &mut App,
) -> AnyElement {
    let mut row = h_flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(6.0))
        .child(metadata_pill("rows", rows_value, cx));
    if let Some(ms) = load_ms {
        row = row.child(metadata_pill("time", format!("{ms} ms"), cx));
    }
    row.child(tab_breadcrumb_read_only_indicator(read_only_id, cx))
        .into_any_element()
}

/// Muted eye icon shown at the trailing edge of data-viewer breadcrumbs.
pub fn tab_breadcrumb_read_only_indicator(id: &'static str, cx: &App) -> AnyElement {
    toolbar_button(id, IconName::Eye, "Read-only", cx)
        .disabled(true)
        .into_any_element()
}

/// Breadcrumb footer for center dock tabs (connection / schema / object).
pub fn tab_breadcrumb_footer(
    id: impl Into<ElementId>,
    crumbs: TabBreadcrumb,
    trailing: Option<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;
    let mono = prefs::code_font_family(cx);
    let ui = prefs::ui_font_family(cx);
    let border = cx.theme().border;
    let last = crumbs.segments.len().saturating_sub(1);

    let mut row = h_flex()
        .id(id)
        .w_full()
        .h(px(TAB_BREADCRUMB_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(4.0))
        .px(px(10.0))
        .border_t_1()
        .border_color(border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.18))
        .child(engine_icon(crumbs.engine));

    for (i, segment) in crumbs.segments.into_iter().enumerate() {
        if i > 0 {
            row = row.child(div().text_xs().text_color(muted.opacity(0.72)).child("/"));
        }
        let is_last = i == last;
        row = row.child(
            div()
                .text_xs()
                .font_family(if i == 0 { ui.clone() } else { mono.clone() })
                .font_weight(if is_last {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_last { fg } else { muted.opacity(0.92) })
                .truncate()
                .child(segment),
        );
    }

    row = row.child(div().flex_1().min_w_0());
    if let Some(trailing) = trailing {
        row = row.child(trailing);
    }

    row
}

/// Center-tab layout: scrollable/flex body with a fixed breadcrumb footer.
pub fn panel_tab_content(body: impl IntoElement, footer: impl IntoElement) -> impl IntoElement {
    v_flex()
        .size_full()
        .min_h_0()
        .child(div().flex_1().min_h_0().child(body))
        .child(footer)
}
