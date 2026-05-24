use std::sync::Arc;

use gpui::{AnyView, App};
use gpui_component::dock::{DockArea, DockItem, PanelView};

pub(crate) fn center_tab_items(item: &DockItem) -> Option<&[Arc<dyn PanelView>]> {
    match item {
        DockItem::Tabs { items, .. } => Some(items),
        DockItem::Split { items, .. } => items.iter().find_map(center_tab_items),
        _ => None,
    }
}

pub(crate) fn active_center_tab(item: &DockItem, _cx: &App) -> Option<(Arc<dyn PanelView>, usize)> {
    match item {
        DockItem::Tabs {
            items, active_ix, ..
        } => {
            let panel = items.get(*active_ix)?.clone();
            Some((panel, items.len()))
        }
        DockItem::Split { items, .. } => {
            items.iter().find_map(|child| active_center_tab(child, _cx))
        }
        _ => None,
    }
}

pub(crate) fn collect_dock_panel_views(item: &DockItem, out: &mut Vec<AnyView>) {
    match item {
        DockItem::Split { items, .. } => {
            for child in items {
                collect_dock_panel_views(child, out);
            }
        }
        DockItem::Tabs { items, .. } => {
            for p in items {
                out.push(p.view());
            }
        }
        DockItem::Panel { view, .. } => {
            out.push(view.view());
        }
        DockItem::Tiles { .. } => {}
    }
}

pub(crate) fn dock_area_present_views(dock: &DockArea, cx: &App) -> Vec<AnyView> {
    let mut v = Vec::new();
    collect_dock_panel_views(dock.center(), &mut v);
    if let Some(left) = dock.left_dock() {
        collect_dock_panel_views(left.read(cx).panel(), &mut v);
    }
    if let Some(right) = dock.right_dock() {
        collect_dock_panel_views(right.read(cx).panel(), &mut v);
    }
    if let Some(bottom) = dock.bottom_dock() {
        collect_dock_panel_views(bottom.read(cx).panel(), &mut v);
    }
    v
}
