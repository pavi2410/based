use std::sync::Arc;

use gpui::{App, Context, Entity, EntityId, Window};
use gpui_component::Placement;
use gpui_component::dock::{
    BasePanelView, DockArea, DockLayout, DockPlacement, InsertTarget, NodeId, PaneNode, PaneRef,
    PanelHandle, PanelId, PanelView,
};

use crate::mongodb::change_stream::ChangeStreamPanel;
use crate::mongodb::document_editor::DocumentEditorPanel;
use crate::mongodb::document_viewer::DocumentViewerPanel;
use crate::mongodb::inspector::CollectionInspectorPanel;
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::mongodb::tree::CollectionsTreePanel;
use crate::postgres::data_viewer::DataViewerPanel as PgDataViewerPanel;
use crate::postgres::inspector::TableInspectorPanel as PgInspectorPanel;
use crate::postgres::query_editor::QueryEditorPanel as PgQueryEditorPanel;
use crate::postgres::tree::SchemaTreePanel as PgSchemaTreePanel;
use crate::sqlite::data_viewer::DataViewerPanel as SqliteDataViewerPanel;
use crate::sqlite::fts_console::FtsConsolePanel;
use crate::sqlite::inspector::TableInspectorPanel as SqliteInspectorPanel;
use crate::sqlite::query_editor::QueryEditorPanel as SqliteQueryEditorPanel;
use crate::sqlite::tree::SchemaTreePanel as SqliteSchemaTreePanel;
use crate::workspace::panels::connection_wizard::ConnectionWizardPanel;
use crate::workspace::panels::home::HomePanel;
use crate::workspace::panels::object_info::{ConnectionDashboardPanel, ObjectInfoPanel};
use crate::workspace::panels::release_notes::ReleaseNotesPanel;

/// Wrap a styled panel so the dock skin can recover titles and menus.
pub(crate) fn dock_handle(panel: Arc<dyn PanelView>) -> Arc<dyn BasePanelView> {
    Arc::new(PanelHandle::from_view(panel))
}

pub(crate) fn tabs_layout(panels: &[Arc<dyn PanelView>], cx: &App) -> DockLayout {
    let mut layout = DockLayout::tabs();
    for panel in panels {
        layout = layout.panel_view(dock_handle(panel.clone()), cx);
    }
    layout
}

pub(crate) fn panel_entity_id(panel: &Arc<dyn PanelView>) -> EntityId {
    panel.view().entity_id()
}

pub(crate) fn to_panel_id(entity_id: EntityId) -> PanelId {
    PanelId::from(entity_id)
}

fn presentation_of(panel: &Arc<dyn BasePanelView>) -> Option<Arc<dyn PanelView>> {
    PanelHandle::of(panel).map(PanelHandle::panel)
}

fn center_tree(dock: &DockArea) -> Option<&gpui_component::dock::PaneTree> {
    dock.layout(DockPlacement::Center)
}

fn walk_tab_groups(node: &PaneNode, f: &mut impl FnMut(NodeId, &[PanelId], usize)) {
    match node.kind() {
        PaneRef::Tabs { panels, active_ix } => f(node.id(), panels, active_ix),
        PaneRef::Split { children, .. } => {
            for child in children {
                walk_tab_groups(child, f);
            }
        }
        PaneRef::Tiles { .. } => {}
    }
}

pub(crate) fn center_tab_group_count(dock: &DockArea) -> usize {
    let Some(tree) = center_tree(dock) else {
        return 0;
    };
    let mut count = 0;
    walk_tab_groups(tree.root(), &mut |_, _, _| count += 1);
    count
}

/// Locate a center tab, the tab-group node that owns it, and its index in that strip.
pub(crate) fn center_panel_by_id(
    dock: &DockArea,
    panel_id: EntityId,
    _cx: &App,
) -> Option<(NodeId, Arc<dyn PanelView>, usize)> {
    let wanted = to_panel_id(panel_id);
    let tree = center_tree(dock)?;
    let mut found = None;
    walk_tab_groups(tree.root(), &mut |node, panels, _| {
        if found.is_some() {
            return;
        }
        if let Some(ix) = panels.iter().position(|id| *id == wanted) {
            found = dock
                .panel(wanted)
                .and_then(presentation_of)
                .map(|view| (node, view, ix));
        }
    });
    found
}

pub(crate) fn active_live_center_panel(dock: &DockArea, _cx: &App) -> Option<Arc<dyn PanelView>> {
    let tree = center_tree(dock)?;
    let mut active = None;
    walk_tab_groups(tree.root(), &mut |_, panels, active_ix| {
        if active.is_some() {
            return;
        }
        if let Some(id) = panels.get(active_ix).copied() {
            active = dock.panel(id).and_then(presentation_of);
        }
    });
    active
}

/// Focus a center tab that is already docked, or add it to the first tab group.
pub(crate) fn activate_center_panel(
    dock: &Entity<DockArea>,
    panel: Arc<dyn PanelView>,
    window: &mut Window,
    cx: &mut App,
) {
    let panel_id = to_panel_id(panel_entity_id(&panel));
    dock.update(cx, |area, cx| {
        let already = area.layout(DockPlacement::Center).and_then(|tree| {
            let node = tree.find_panel_node(panel_id)?;
            let mut ix = None;
            walk_tab_groups(tree.root(), &mut |id, panels, _| {
                if id == node {
                    ix = panels.iter().position(|p| *p == panel_id);
                }
            });
            ix.map(|ix| (node, ix))
        });
        if let Some((node, ix)) = already {
            area.move_panel(
                panel_id,
                InsertTarget::Tabs {
                    node,
                    ix: Some(ix),
                    activate: true,
                },
                window,
                cx,
            );
            return;
        }
        area.add_panel_view(dock_handle(panel), DockPlacement::Center, None, window, cx);
    });
}

pub(crate) fn add_center_panel_view(
    dock: &Entity<DockArea>,
    panel: Arc<dyn PanelView>,
    window: &mut Window,
    cx: &mut App,
) {
    dock.update(cx, |area, cx| {
        area.add_panel_view(dock_handle(panel), DockPlacement::Center, None, window, cx);
    });
}

pub(crate) fn split_center_with_panel(
    dock: &Entity<DockArea>,
    panel: Arc<dyn PanelView>,
    placement: Placement,
    window: &mut Window,
    cx: &mut App,
) {
    let new_id = to_panel_id(panel_entity_id(&panel));
    dock.update(cx, |area, cx| {
        let active_node = area.layout(DockPlacement::Center).and_then(|tree| {
            let mut node = None;
            walk_tab_groups(tree.root(), &mut |id, panels, active_ix| {
                if node.is_some() {
                    return;
                }
                if panels.get(active_ix).is_some() {
                    node = Some(id);
                }
            });
            node
        });
        area.add_panel_view(dock_handle(panel), DockPlacement::Center, None, window, cx);
        if let Some(node) = active_node {
            area.move_panel(
                new_id,
                InsertTarget::Split {
                    node,
                    placement,
                    size: None,
                },
                window,
                cx,
            );
        }
    });
}

/// `DockArea::remove_panel` is typed; recover the entity from the live view.
pub(crate) fn remove_presentation_panel(
    dock: &mut DockArea,
    panel: &Arc<dyn PanelView>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let view = panel.view();
    macro_rules! attempt {
        ($($ty:ty),+ $(,)?) => {{
            $(
                if let Ok(entity) = view.clone().downcast::<$ty>() {
                    dock.remove_panel(entity, window, cx);
                    return;
                }
            )+
        }};
    }
    attempt!(
        HomePanel,
        ConnectionWizardPanel,
        ReleaseNotesPanel,
        ConnectionDashboardPanel,
        ObjectInfoPanel,
        ChangeStreamPanel,
        DocumentEditorPanel,
        DocumentViewerPanel,
        CollectionInspectorPanel,
        PipelineBuilderPanel,
        CollectionsTreePanel,
        PgQueryEditorPanel,
        PgDataViewerPanel,
        PgInspectorPanel,
        PgSchemaTreePanel,
        SqliteQueryEditorPanel,
        SqliteDataViewerPanel,
        SqliteInspectorPanel,
        SqliteSchemaTreePanel,
        FtsConsolePanel,
    );
    log::error!(
        "unrecognized center panel type for remove: {}",
        panel.panel_name(cx)
    );
}

pub(crate) fn tab_group_panel_ids(dock: &DockArea, node: NodeId) -> Vec<PanelId> {
    let Some(tree) = center_tree(dock) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    walk_tab_groups(tree.root(), &mut |id, panels, _| {
        if id == node {
            ids.extend(panels.iter().copied());
        }
    });
    ids
}
