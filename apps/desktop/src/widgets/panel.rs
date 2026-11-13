//! Panel chrome — shell, headers, and toolbar components for boxed content panels.

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::connection::{ConnectionId, EngineKind, Environment};
use crate::widgets::engine_icon;
use crate::widgets::layout::panel_header_height;
use crate::widgets::metadata_pill::env_pill;
use crate::widgets::status_item::STATUS_BAR_HEIGHT;
use crate::workspace::WorkspaceRef;

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

/// Height of the data-viewer filter/export toolbar (centers compact controls).
pub const DATA_VIEWER_TOOLBAR_H: f32 = 28.0;

/// Compact filter/export toolbar with symmetric vertical centering.
pub fn data_viewer_toolbar(cx: &mut App) -> Div {
    h_flex()
        .w_full()
        .flex_shrink_0()
        .min_h(px(DATA_VIEWER_TOOLBAR_H))
        .items_center()
        .px(px(8.0))
        .py(px(4.0))
        .gap(px(6.0))
        .flex_wrap()
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.18))
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
    /// Marked on the connection; production tints the whole footer.
    pub environment: Option<Environment>,
    pub read_only: bool,
}

/// Where the tab's queries run: connection, database, then `tail` (schema, table, …).
/// This footer is the one place a tab names its connection, so it carries the
/// environment and read-only state too.
pub fn tab_breadcrumb_for_connection(
    conn_id: &ConnectionId,
    tail: impl IntoIterator<Item = impl Into<SharedString>>,
    cx: &App,
) -> TabBreadcrumb {
    // The workspace's registry, which exists with or without an open project.
    let entry = cx
        .try_global::<WorkspaceRef>()
        .and_then(|ws| ws.0.read(cx).registry().read(cx).get(conn_id, cx))
        .map(|e| e.read(cx));
    let Some(entry) = entry else {
        let mut segments = vec![conn_id.to_string().into()];
        segments.extend(tail.into_iter().map(Into::into));
        return TabBreadcrumb {
            engine: EngineKind::Postgres,
            segments,
            environment: None,
            read_only: false,
        };
    };
    let mut segments: Vec<SharedString> = vec![entry.label.clone().into()];
    segments.extend(entry.database_label(cx).map(Into::into));
    segments.extend(tail.into_iter().map(Into::into));
    TabBreadcrumb {
        engine: entry.engine(),
        segments,
        environment: entry.environment(),
        read_only: entry.config.is_read_only(),
    }
}

/// Trailing cluster for data-viewer breadcrumbs: pagination, load time, optional read-only indicator.
pub fn tab_breadcrumb_data_viewer_trailing(
    pagination: impl IntoElement,
    load_ms: Option<u64>,
    fetched_ago: Option<String>,
    refresh: impl IntoElement,
    show_read_only: bool,
    read_only_id: &'static str,
    cx: &mut App,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let text = |s: String| div().text_xs().text_color(muted).child(s);
    let mut row = h_flex().flex_shrink_0().items_center().gap(px(10.0));
    if let Some(ms) = load_ms {
        row = row.child(text(format!("{ms} ms")));
    }
    if let Some(ago) = fetched_ago {
        row = row.child(text(ago));
    }
    row = row.child(refresh);
    if show_read_only {
        row = row.child(tab_breadcrumb_read_only_indicator(read_only_id, cx));
    }
    row.child(pagination).into_any_element()
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

    let production = crumbs.environment == Some(Environment::Production);
    let danger = cx.theme().danger;
    let (border, bg) = if production {
        (danger.opacity(0.6), danger.opacity(0.12))
    } else {
        (border.opacity(0.72), cx.theme().muted.opacity(0.18))
    };
    let mut row = h_flex()
        .id(id)
        .w_full()
        .h(px(TAB_BREADCRUMB_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(4.0))
        .px(px(10.0))
        .border_t_1()
        .border_color(border)
        .bg(bg)
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
        if i == 0 {
            if let Some(env) = crumbs.environment {
                row = row.child(env_pill(env, cx));
            }
            if crumbs.read_only {
                row = row.child(div().text_xs().text_color(muted).child("read-only"));
            }
        }
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
