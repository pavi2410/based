use std::sync::Arc;

use gpui::{AnyView, App, Entity, EntityId, WeakEntity, Window};
use gpui_component::dock::{DockArea, DockItem, PanelView, TabPanel};

/// Wrap a center tab group in a vertical split root so its `TabPanel` receives a
/// `StackPanel` parent (required for split-pane and drag-split in gpui-component).
pub(crate) fn wrap_center_root(
    tabs: DockItem,
    dock_area: &WeakEntity<DockArea>,
    window: &mut Window,
    cx: &mut App,
) -> DockItem {
    DockItem::v_split(vec![tabs], dock_area, window, cx)
}

pub(crate) fn center_tab_items(item: &DockItem) -> Option<&[Arc<dyn PanelView>]> {
    match item {
        DockItem::Tabs { items, .. } => Some(items),
        DockItem::Split { items, .. } => items.iter().find_map(center_tab_items),
        _ => None,
    }
}

pub(crate) fn center_tab_items_for_panel<'a>(
    item: &'a DockItem,
    tab_panel: &Entity<TabPanel>,
) -> Option<&'a [Arc<dyn PanelView>]> {
    match item {
        DockItem::Tabs { items, view, .. } if view == tab_panel => Some(items),
        DockItem::Split { items, .. } => items
            .iter()
            .find_map(|child| center_tab_items_for_panel(child, tab_panel)),
        _ => None,
    }
}

pub(crate) fn center_tab_panels(item: &DockItem, out: &mut Vec<Entity<TabPanel>>) {
    match item {
        DockItem::Tabs { view, .. } => out.push(view.clone()),
        DockItem::Split { items, .. } => {
            for child in items {
                center_tab_panels(child, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn center_tab_panel_count(item: &DockItem) -> usize {
    let mut out = Vec::new();
    center_tab_panels(item, &mut out);
    out.len()
}

/// The `TabPanel` that owns the primary active center tab (first match in tree order).
pub(crate) fn active_center_tab_panel(item: &DockItem) -> Option<Entity<TabPanel>> {
    match item {
        DockItem::Tabs {
            view,
            items,
            active_ix,
            ..
        } if items.get(*active_ix).is_some() => Some(view.clone()),
        DockItem::Split { items, .. } => items.iter().find_map(active_center_tab_panel),
        _ => None,
    }
}

/// Locate a center tab panel, its panel view, and index within that strip.
pub(crate) fn center_panel_by_id(
    item: &DockItem,
    panel_id: EntityId,
    cx: &App,
) -> Option<(Entity<TabPanel>, Arc<dyn PanelView>, usize)> {
    match item {
        DockItem::Tabs { items, view, .. } => {
            let (ix, panel) = items
                .iter()
                .enumerate()
                .find(|(_, p)| p.panel_id(cx) == panel_id)?;
            Some((view.clone(), panel.clone(), ix))
        }
        DockItem::Split { items, .. } => items
            .iter()
            .find_map(|child| center_panel_by_id(child, panel_id, cx)),
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
