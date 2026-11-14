//! Finding a window's workspace, and the tab helpers panels call.

use std::path::Path;

use gpui_kit::{App, Entity, EntityId, Global, WeakEntity, Window, WindowId};

use super::inbox::{Inbox, Request};
use super::{TabEvent, TabManager};
use crate::connection::registry::ConnectionRegistry;
use crate::project::scope::ProjectState;
use crate::workspace::Workspace;
use gpui_kit::component::dock::DockArea;

/// One open workspace window and the handles panels reach it through. The tab
/// manager, dock and registry are kept here as well so a panel can find them
/// while its workspace is in the middle of an update.
struct WorkspaceWindow {
    window: WindowId,
    workspace: WeakEntity<Workspace>,
    tabs: Entity<TabManager>,
    dock: Entity<DockArea>,
    registry: Entity<ConnectionRegistry>,
    /// Requests for this window's workspace, applied on its next render.
    inbox: Inbox,
    /// The project this window has open (or none).
    project: ProjectState,
}

/// Every open workspace window. A workspace belongs to one window and one
/// project; code that has a window asks for that window's workspace, code acting
/// for the user asks for the focused one, and app-wide work visits them all.
#[derive(Default)]
pub struct Workspaces {
    windows: Vec<WorkspaceWindow>,
    /// The workspace window that last had focus. Menus, pop-outs and the
    /// Settings window act for it while they hold focus.
    last_focused: Option<WindowId>,
}

impl Global for Workspaces {}

impl Workspaces {
    pub fn register(
        window: WindowId,
        workspace: WeakEntity<Workspace>,
        tabs: Entity<TabManager>,
        dock: Entity<DockArea>,
        registry: Entity<ConnectionRegistry>,
        project: ProjectState,
        cx: &mut App,
    ) {
        cx.default_global::<Workspaces>()
            .windows
            .push(WorkspaceWindow {
                window,
                workspace,
                tabs,
                dock,
                registry,
                inbox: Inbox::default(),
                project,
            });
    }

    /// The project open in `window`.
    pub fn project(window: WindowId, cx: &App) -> Option<&ProjectState> {
        Self::entries(cx)
            .iter()
            .find(|w| w.window == window)
            .map(|w| &w.project)
    }

    /// Change the project state of `window`.
    pub fn update_project<R>(
        window: WindowId,
        cx: &mut App,
        f: impl FnOnce(&mut ProjectState) -> R,
    ) -> Option<R> {
        if !cx.has_global::<Workspaces>() {
            return None;
        }
        cx.global_mut::<Workspaces>()
            .windows
            .iter_mut()
            .find(|w| w.window == window)
            .map(|w| f(&mut w.project))
    }

    /// The window that owns `panel_id`, else the active one.
    pub fn window_for_panel(panel_id: EntityId, cx: &App) -> Option<WindowId> {
        Self::entry_for_panel_or_active(panel_id, cx).map(|w| w.window)
    }

    /// The project of the window that owns `panel_id`, else the active one's.
    pub fn project_for_panel(panel_id: EntityId, cx: &App) -> Option<&ProjectState> {
        Self::entry_for_panel_or_active(panel_id, cx).map(|w| &w.project)
    }

    /// The project of the window the user is acting in.
    pub fn active_project(cx: &App) -> Option<&ProjectState> {
        Self::active_entry(cx).map(|w| &w.project)
    }

    /// The window that has `root` open, if any.
    pub fn window_with_project(root: &Path, cx: &App) -> Option<WindowId> {
        Self::entries(cx)
            .iter()
            .find(|w| w.project.root() == Some(root))
            .map(|w| w.window)
    }

    /// Queue `request` for the workspace in `window`.
    pub fn post_to(window: WindowId, request: Request, cx: &mut App) {
        if !cx.has_global::<Workspaces>() {
            return;
        }
        if let Some(entry) = cx
            .global_mut::<Workspaces>()
            .windows
            .iter_mut()
            .find(|w| w.window == window)
        {
            entry.inbox.push(request);
        }
    }

    /// The next queued request for the workspace in `window`.
    pub fn take_request(window: WindowId, cx: &mut App) -> Option<Request> {
        if !cx.has_global::<Workspaces>() {
            return None;
        }
        cx.global_mut::<Workspaces>()
            .windows
            .iter_mut()
            .find(|w| w.window == window)?
            .inbox
            .pop()
    }

    /// The window of the workspace the user is acting in.
    pub fn active_window(cx: &App) -> Option<WindowId> {
        Self::active_entry(cx).map(|w| w.window)
    }

    /// The window whose workspace owns `registry`.
    pub fn window_with_registry(
        registry: &Entity<ConnectionRegistry>,
        cx: &App,
    ) -> Option<WindowId> {
        Self::entries(cx)
            .iter()
            .find(|w| w.registry == *registry)
            .map(|w| w.window)
    }

    /// Forget `window`. Returns whether it was a workspace window.
    pub fn unregister(window: WindowId, cx: &mut App) -> bool {
        if !cx.has_global::<Workspaces>() {
            return false;
        }
        let windows = &mut cx.global_mut::<Workspaces>().windows;
        let before = windows.len();
        windows.retain(|w| w.window != window);
        windows.len() != before
    }

    /// No workspace window is open.
    pub fn is_empty(cx: &App) -> bool {
        Self::entries(cx).is_empty()
    }

    fn entries(cx: &App) -> &[WorkspaceWindow] {
        cx.try_global::<Workspaces>()
            .map(|all| all.windows.as_slice())
            .unwrap_or_default()
    }

    /// Every live workspace, oldest first.
    pub fn all(cx: &App) -> Vec<Entity<Workspace>> {
        Self::entries(cx)
            .iter()
            .filter_map(|w| w.workspace.upgrade())
            .collect()
    }

    /// The workspace shown in `window`.
    pub fn for_window(window: &Window, cx: &App) -> Option<Entity<Workspace>> {
        Self::for_window_id(window.window_handle().window_id(), cx)
    }

    pub fn for_window_id(window: WindowId, cx: &App) -> Option<Entity<Workspace>> {
        Self::entries(cx)
            .iter()
            .find(|w| w.window == window)
            .and_then(|w| w.workspace.upgrade())
    }

    /// `window` has focus (called from its render).
    pub fn note_focused(window: WindowId, cx: &mut App) {
        if let Some(all) = cx.try_global::<Workspaces>()
            && all.last_focused != Some(window)
        {
            cx.global_mut::<Workspaces>().last_focused = Some(window);
        }
    }

    fn active_entry(cx: &App) -> Option<&WorkspaceWindow> {
        let entries = Self::entries(cx);
        let find = |id: WindowId| entries.iter().find(|w| w.window == id);
        cx.active_window()
            .and_then(|a| find(a.window_id()))
            .or_else(|| {
                let last = cx.try_global::<Workspaces>()?.last_focused?;
                find(last)
            })
            .or_else(|| entries.last())
    }

    /// The workspace the user is acting in: the focused window's, else the most
    /// recently opened one (a menu or dialog may hold focus).
    pub fn active(cx: &App) -> Option<Entity<Workspace>> {
        Self::active_entry(cx).and_then(|w| w.workspace.upgrade())
    }

    fn entry_for_panel(panel_id: EntityId, cx: &App) -> Option<&WorkspaceWindow> {
        Self::entries(cx)
            .iter()
            .find(|w| w.tabs.read(cx).tab_for_panel_id(panel_id).is_some())
    }

    /// The panel's entry, else the active one (a panel still being built, or one
    /// that is not a tab, such as the chat pane).
    fn entry_for_panel_or_active(panel_id: EntityId, cx: &App) -> Option<&WorkspaceWindow> {
        Self::entry_for_panel(panel_id, cx).or_else(|| Self::active_entry(cx))
    }

    /// The workspace whose tabs include the panel `panel_id`.
    pub fn with_panel(panel_id: EntityId, cx: &App) -> Option<Entity<Workspace>> {
        Self::entry_for_panel(panel_id, cx).and_then(|w| w.workspace.upgrade())
    }

    /// The workspace that owns `panel_id`, else the active one.
    pub fn for_panel(panel_id: EntityId, cx: &App) -> Option<Entity<Workspace>> {
        Self::entry_for_panel_or_active(panel_id, cx).and_then(|w| w.workspace.upgrade())
    }

    /// The tab manager holding `panel_id`.
    pub fn tabs_for_panel(panel_id: EntityId, cx: &App) -> Option<Entity<TabManager>> {
        Self::entry_for_panel(panel_id, cx).map(|w| w.tabs.clone())
    }

    /// The center dock holding `panel_id`.
    pub fn dock_for_panel(panel_id: EntityId, cx: &App) -> Option<Entity<DockArea>> {
        Self::entry_for_panel(panel_id, cx).map(|w| w.dock.clone())
    }

    /// The connections of the workspace that owns `panel_id`, else the active one's.
    pub fn registry_for_panel(panel_id: EntityId, cx: &App) -> Option<Entity<ConnectionRegistry>> {
        Self::entry_for_panel_or_active(panel_id, cx).map(|w| w.registry.clone())
    }

    /// The connections of the workspace in `window`.
    pub fn registry_for_window(window: WindowId, cx: &App) -> Option<Entity<ConnectionRegistry>> {
        Self::entries(cx)
            .iter()
            .find(|w| w.window == window)
            .map(|w| w.registry.clone())
    }

    /// The connections of the workspace the user is acting in.
    pub fn active_registry(cx: &App) -> Option<Entity<ConnectionRegistry>> {
        Self::active_entry(cx).map(|w| w.registry.clone())
    }
}

/// Clear the unsaved-edits mark on the tab showing this panel.
pub fn clear_tab_dirty(panel_id: EntityId, cx: &mut App) {
    let Some(handle) = Workspaces::tabs_for_panel(panel_id, cx) else {
        return;
    };
    handle.update(cx, |tm, cx| {
        if let Some(tab) = tm.tab_for_panel_id_mut(panel_id)
            && tab.dirty
        {
            tab.dirty = false;
            cx.notify();
        }
    });
}

/// Tell the panel's workspace a tab's content changed, so it saves the session once edits pause.
pub fn note_tab_edited(panel_id: EntityId, cx: &mut App) {
    if let Some(handle) = Workspaces::tabs_for_panel(panel_id, cx) {
        handle.update(cx, |_, cx| cx.emit(TabEvent::Edited));
    }
}

/// Mark the tab showing this panel as having unsaved edits.
pub fn mark_tab_dirty(panel_id: EntityId, cx: &mut App) {
    let Some(handle) = Workspaces::tabs_for_panel(panel_id, cx) else {
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
