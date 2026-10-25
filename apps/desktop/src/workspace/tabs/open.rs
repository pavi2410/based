//! Globals that let panels and chrome reach the workspace's tab state.

use gpui_kit::{App, Entity, EntityId, Global};

use super::TabManager;
use crate::workspace::Workspace;
use gpui_kit::component::dock::DockArea;

#[derive(Clone)]
pub struct TabManagerRef(pub Entity<TabManager>);

impl Global for TabManagerRef {}

/// Center dock area (tab strip) for chrome that must not read [`Workspace`](crate::workspace::Workspace).
#[derive(Clone)]
pub struct DockAreaRef(pub Entity<DockArea>);

impl Global for DockAreaRef {}

/// Main workspace entity (tab close menu, ⌘W).
#[derive(Clone)]
pub struct WorkspaceRef(pub Entity<Workspace>);

impl Global for WorkspaceRef {}

/// Mark the tab showing this panel as having unsaved edits.
pub fn mark_tab_dirty(panel_id: EntityId, cx: &mut App) {
    let Some(handle) = cx.try_global::<TabManagerRef>().map(|h| h.0.clone()) else {
        return;
    };
    handle.update(cx, |tm, cx| {
        if let Some(tab) = tm.tab_for_panel_id_mut(panel_id)
            && !tab.dirty
        {
            tab.dirty = true;
            cx.notify();
        }
    });
}
