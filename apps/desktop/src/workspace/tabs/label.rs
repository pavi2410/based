//! Tab strip labels (`Panel::title` when `tab_name` is unset).

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, Theme,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::ContextMenuExt,
    tooltip::Tooltip,
};
use gpui_kit::{App, EntityId, Hsla, IntoElement, SharedString, div, prelude::*, px};

use crate::workspace::WorkspaceRef;

use super::CloseScope;
use super::kind::PanelKind;
use super::menu::tab_context_menu;
use super::open::TabManagerRef;
use super::spec::TabSpec;

/// Longest a tab may grow before its label is truncated.
pub const TAB_MAX_WIDTH: f32 = 120.0;

pub const PIN_ICON_PATH: &str = "icons/pin.svg";

pub fn is_panel_pinned(panel_id: EntityId, cx: &App) -> bool {
    cx.try_global::<TabManagerRef>()
        .and_then(|tm| tm.0.read(cx).tab_for_panel_id(panel_id))
        .is_some_and(|t| t.pinned)
}

pub fn render_strip_tab(
    label: SharedString,
    dirty: bool,
    panel_id: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let pinned = is_panel_pinned(panel_id, cx);
    let spec = tab_spec(panel_id, cx);
    let icon = spec.as_ref().map(|s| tab_icon(s, cx.theme()));
    let info = tab_info(&label, spec.as_ref());
    // The dirty marker is drawn as a small dot instead of a text glyph.
    let dirty = dirty || label.ends_with(DIRTY_MARKER);
    let text: SharedString = label
        .strip_suffix(DIRTY_MARKER)
        .unwrap_or(&label)
        .to_string()
        .into();
    let color = cx.theme().tab_foreground;

    h_flex()
        .id(("tab-title", panel_id.as_u64()))
        .max_w(px(TAB_MAX_WIDTH))
        .tooltip(move |window, cx| Tooltip::new(info.clone()).build(window, cx))
        .context_menu(move |menu, _window, cx| tab_context_menu(menu, panel_id, cx))
        .gap(px(4.0))
        .items_center()
        .when_some(icon, |this, (name, color)| {
            this.child(Icon::new(name).xsmall().text_color(color))
        })
        .when(pinned, |this| {
            this.child(
                Icon::empty()
                    .path(PIN_ICON_PATH)
                    .xsmall()
                    .text_color(color.opacity(0.75)),
            )
        })
        .child(div().min_w_0().truncate().child(text))
        .when(dirty, |this| {
            this.child(
                div()
                    .size(px(6.0))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(color.opacity(0.8)),
            )
        })
        .when(!pinned, |this| this.child(close_button(panel_id, color)))
}

fn tab_spec(panel_id: EntityId, cx: &App) -> Option<TabSpec> {
    cx.try_global::<TabManagerRef>()
        .and_then(|tm| tm.0.read(cx).tab_for_panel_id(panel_id))
        .map(|t| t.spec.clone())
}

/// Leading glyph and tint that identify a tab's kind at a glance.
fn tab_icon(spec: &TabSpec, theme: &Theme) -> (IconName, Hsla) {
    let muted = theme.muted_foreground;
    match spec {
        TabSpec::Home => (IconName::Globe, muted),
        TabSpec::Dashboard { .. } => (IconName::ChartPie, theme.blue_light),
        TabSpec::DataViewer { .. } => (IconName::LayoutDashboard, theme.blue_light),
        TabSpec::Inspector { .. } => (IconName::Inspector, theme.blue_light),
        TabSpec::QueryEditor { .. } => (IconName::SquareTerminal, theme.green_light),
        TabSpec::Pipeline { .. } => (IconName::GalleryVerticalEnd, theme.green_light),
        TabSpec::DocumentInsert { .. } => (IconName::Plus, theme.green_light),
        TabSpec::ObjectInfo { .. } => (IconName::Info, muted),
        TabSpec::ReleaseNotes { .. } => (IconName::FileText, muted),
        TabSpec::ConnectionWizard { .. } => kind_icon(PanelKind::Wizard, theme),
        TabSpec::Transient { kind, .. } => kind_icon(*kind, theme),
    }
}

fn kind_icon(kind: PanelKind, theme: &Theme) -> (IconName, Hsla) {
    let (blue, green, red) = (theme.blue_light, theme.green_light, theme.red_light);
    let muted = theme.muted_foreground;
    match kind {
        PanelKind::DataViewer => (IconName::LayoutDashboard, blue),
        PanelKind::Inspector => (IconName::Inspector, blue),
        PanelKind::DocumentViewer => (IconName::Inbox, green),
        PanelKind::DocumentEditor => (IconName::Plus, green),
        PanelKind::ChangeStream => (IconName::Redo2, green),
        PanelKind::RedisConsole => (IconName::SquareTerminal, red),
        PanelKind::Redis => (IconName::Asterisk, red),
        PanelKind::ObjectInfo => (IconName::Info, muted),
        PanelKind::Home
        | PanelKind::Dashboard
        | PanelKind::Wizard
        | PanelKind::ReleaseNotes
        | PanelKind::QueryEditor
        | PanelKind::Pipeline
        | PanelKind::Other => (IconName::Settings, muted),
    }
}

/// Hover card text: tab title, what it shows, and its connection.
fn tab_info(label: &SharedString, spec: Option<&TabSpec>) -> SharedString {
    let Some(spec) = spec else {
        return label.clone();
    };
    let detail = match spec {
        TabSpec::DataViewer { object, .. } => format!("Table data · {object}"),
        TabSpec::Inspector { object, .. } => format!("Schema · {object}"),
        TabSpec::QueryEditor { .. } => "Query editor".into(),
        TabSpec::Pipeline { collection, .. } => format!("Aggregation pipeline · {collection}"),
        TabSpec::ObjectInfo { object, kind, .. } => format!("{} · {object}", kind.label()),
        TabSpec::DocumentInsert { collection, .. } => format!("Insert document · {collection}"),
        TabSpec::Dashboard { .. } => "Connection dashboard".into(),
        TabSpec::Home
        | TabSpec::ReleaseNotes { .. }
        | TabSpec::ConnectionWizard { .. }
        | TabSpec::Transient { .. } => String::new(),
    };
    let mut lines = vec![label.to_string()];
    if !detail.is_empty() {
        lines.push(detail);
    }
    if let Some(conn) = spec.conn_id() {
        lines.push(format!("Connection · {conn}"));
    }
    lines.join("\n").into()
}

/// Per-tab × that closes through the workspace so pin and Home rules still apply.
fn close_button(panel_id: EntityId, color: gpui_kit::Hsla) -> impl IntoElement {
    Button::new(("tab-close", panel_id.as_u64()))
        .ghost()
        .xsmall()
        .icon(Icon::new(IconName::Close).text_color(color.opacity(0.6)))
        .tooltip("Close tab")
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                ws.update(cx, |ws, cx| {
                    ws.close_tabs(CloseScope::Tab(panel_id), window, cx);
                });
            }
        })
}

pub fn tab_label_for_spec(spec: &TabSpec, dirty: bool) -> SharedString {
    let base = match spec {
        TabSpec::Home => "Home".to_string(),
        TabSpec::Dashboard { conn_id } => conn_id.to_string(),
        TabSpec::DataViewer { object, filter, .. } => match filter {
            Some(f) => format!("{} · {}={}", object.name, f.column, f.value),
            None => object.name.clone(),
        },
        TabSpec::QueryEditor { .. } => "Query".to_string(),
        TabSpec::Pipeline { collection, .. } => collection.clone(),
        TabSpec::Inspector { object, .. } => format!("{} (schema)", object.name),
        TabSpec::ObjectInfo { object, kind, .. } => format!("{} ({})", object.name, kind.label()),
        TabSpec::DocumentInsert { collection, .. } => format!("Insert · {collection}"),
        TabSpec::ReleaseNotes { version } => format!("What's New in v{version}"),
        TabSpec::ConnectionWizard { .. } => PanelKind::Wizard.label().to_string(),
        TabSpec::Transient { kind, .. } => kind.label().to_string(),
    };
    with_dirty_suffix(base, dirty)
}

pub fn with_dirty_suffix(label: impl Into<SharedString>, dirty: bool) -> SharedString {
    let mut s = label.into().to_string();
    if dirty {
        push_dirty_marker(&mut s);
    }
    s.into()
}

const DIRTY_MARKER: &str = " ●";

fn push_dirty_marker(s: &mut String) {
    if !s.ends_with(DIRTY_MARKER) {
        s.push_str(DIRTY_MARKER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    #[test]
    fn data_viewer_short_name() {
        let spec = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("x"),
            object: based_core::ObjectRef::qualified("public", "orders"),
            filter: None,
        };
        assert_eq!(tab_label_for_spec(&spec, false).as_ref(), "orders");
    }

    #[test]
    fn dirty_marker() {
        let s = with_dirty_suffix("Query", true);
        assert_eq!(s.as_ref(), "Query ●");
    }
}
