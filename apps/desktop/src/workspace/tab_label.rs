//! Tab strip labels (`Panel::title` when `tab_name` is unset).

use gpui::{App, EntityId, IntoElement, SharedString, prelude::*, px};
use gpui_component::{ActiveTheme, Icon, Sizable as _, h_flex};

use super::tab_open::TabManagerRef;
use super::tab_spec::TabSpec;

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
    let text = with_dirty_suffix(label, dirty);
    let color = cx.theme().tab_foreground;

    h_flex()
        .gap(px(4.0))
        .items_center()
        .when(pinned, |this| {
            this.child(
                Icon::empty()
                    .path(PIN_ICON_PATH)
                    .xsmall()
                    .text_color(color.opacity(0.75)),
            )
        })
        .child(text)
}

pub fn tab_label_for_spec(spec: &TabSpec, dirty: bool) -> SharedString {
    let base = match spec {
        TabSpec::Welcome => "Welcome".to_string(),
        TabSpec::Dashboard(id) => id.0.clone(),
        TabSpec::DataViewer { object, .. } => short_object_name(object),
        TabSpec::QueryEditor { .. } => "Query".to_string(),
        TabSpec::Pipeline { collection, .. } => collection.clone(),
        TabSpec::Inspector { object, .. } => format!("{} (schema)", short_object_name(object)),
        TabSpec::ObjectInfo {
            object_name,
            kind_label,
            ..
        } => format!("{object_name} ({kind_label})"),
        TabSpec::DocumentInsert { collection, .. } => format!("Insert · {collection}"),
        TabSpec::ReleaseNotes { version } => format!("What's New in v{version}"),
        TabSpec::Builtin { panel, .. } => panel.clone(),
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

fn push_dirty_marker(s: &mut String) {
    const MARKER: &str = " ●";
    if !s.ends_with(MARKER) {
        s.push_str(MARKER);
    }
}

fn short_object_name(object: &str) -> String {
    object
        .rsplit_once('.')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| object.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    #[test]
    fn data_viewer_short_name() {
        let spec = TabSpec::DataViewer {
            conn_id: ConnectionId("x".into()),
            object: "public.orders".into(),
        };
        assert_eq!(tab_label_for_spec(&spec, false).as_ref(), "orders");
    }

    #[test]
    fn dirty_marker() {
        let s = with_dirty_suffix("Query", true);
        assert_eq!(s.as_ref(), "Query ●");
    }
}
