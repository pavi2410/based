//! Center dock panel registry — tracks live panels, manages Home tab, and keeps TabManager in sync.

use std::slice;
use std::sync::Arc;

use gpui_kit::component::dock::{Panel, PanelView};
use gpui_kit::{App, Context, Entity, EntityId, Focusable, Window, prelude::*};

use super::Workspace;
use super::tabs::infer;
use super::tabs::manager::DockTab;
use crate::connection::ConnectionId;
use crate::workspace::panels::ConnectionWizardPanel;
use crate::workspace::wizard_logic::can_edit_saved_connection;

use super::dock_utils::{
    activate_center_panel, active_live_center_panel, add_center_panel_view, panel_entity_id,
};

impl Workspace {
    /// Open the New connection form in a center tab.
    pub fn open_connection_picker_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.new(|cx| ConnectionWizardPanel::new(window, cx));
        self.add_center_panel(panel, window, cx);
    }

    /// Open or focus the edit form for a saved connection.
    pub fn open_edit_connection_tab(
        &mut self,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !can_edit_saved_connection(&conn_id) {
            return;
        }
        if let Some(panel) = self.find_edit_wizard_panel(&conn_id, cx) {
            activate_center_panel(&self.dock_area, panel, window, cx);
            cx.notify();
            return;
        }
        let Some(entry) = self.registry.read(cx).get(&conn_id, cx).cloned() else {
            return;
        };
        let panel = cx.new(|cx| ConnectionWizardPanel::edit(entry, window, cx));
        self.add_center_panel(panel, window, cx);
    }

    fn find_edit_wizard_panel(
        &self,
        conn_id: &ConnectionId,
        cx: &App,
    ) -> Option<Arc<dyn PanelView>> {
        for panel in self.tab_manager.read(cx).panels() {
            if panel.panel_name(cx) != "ConnectionWizard" {
                continue;
            }
            let Ok(ent) = panel.view().downcast::<ConnectionWizardPanel>() else {
                continue;
            };
            if ent.read(cx).editing_id() == Some(conn_id) {
                return Some(panel.clone());
            }
        }
        None
    }

    fn add_center_panel<T: Panel>(
        &mut self,
        panel: Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arc: Arc<dyn PanelView> = Arc::new(panel);
        add_center_panel_view(&self.dock_area, arc.clone(), window, cx);
        self.register_center_panel(arc, cx);
        cx.notify();
    }

    pub(crate) fn register_center_panel(
        &mut self,
        panel: Arc<dyn PanelView>,
        cx: &mut Context<Self>,
    ) {
        let mut panels = self.tab_manager.read(cx).panels();
        if !panels
            .iter()
            .any(|p| panel_entity_id(p) == panel_entity_id(&panel))
        {
            panels.push(panel);
            self.sync_panels(panels, cx);
        }
    }

    pub(crate) fn replace_center_panels(
        &mut self,
        panels: Vec<Arc<dyn PanelView>>,
        cx: &mut Context<Self>,
    ) {
        self.sync_panels(panels, cx);
    }

    pub(crate) fn unregister_center_panel(&mut self, panel_id: EntityId, cx: &mut Context<Self>) {
        let mut panels = self.tab_manager.read(cx).panels();
        panels.retain(|p| panel_entity_id(p) != panel_id);
        self.sync_panels(panels, cx);
    }

    pub(crate) fn find_center_panel(
        &self,
        panel_id: EntityId,
        cx: &App,
    ) -> Option<Arc<dyn PanelView>> {
        let tm = self.tab_manager.read(cx);
        tm.tab_for_panel_id(panel_id).map(|t| t.panel.clone())
    }

    /// Rebuild the center dock with a single Home tab (used after the last tab closes).
    fn reset_center_to_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home_arc: Arc<dyn PanelView> = Arc::new(self.home_panel.clone());
        self.dock_area.update(cx, |dock, ecx| {
            dock.set_center(
                super::dock_utils::tabs_layout(slice::from_ref(&home_arc), ecx),
                window,
                ecx,
            );
        });
        self.replace_center_panels(vec![home_arc], cx);
        self.home_panel.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Focus the Home tab, re-adding it to the center strip if it was replaced.
    pub fn show_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home_id = self.home_panel.entity_id();
        let home_present = self.find_center_panel(home_id, cx).is_some();

        if home_present {
            let home = self
                .find_center_panel(home_id, cx)
                .unwrap_or_else(|| Arc::new(self.home_panel.clone()) as Arc<dyn PanelView>);
            activate_center_panel(&self.dock_area, home, window, cx);
        } else {
            let has_other_live = {
                let dock = self.dock_area.read(cx);
                active_live_center_panel(dock, cx).is_some_and(|p| panel_entity_id(&p) != home_id)
            };
            if has_other_live {
                let panel: Arc<dyn PanelView> = Arc::new(self.home_panel.clone());
                add_center_panel_view(&self.dock_area, panel.clone(), window, cx);
                self.register_center_panel(panel, cx);
            } else {
                self.reset_center_to_home(window, cx);
                return;
            }
        }

        self.home_panel.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// If the center strip has no visible tabs, rebuild Home (Chrome new-tab respawn).
    pub fn ensure_home_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let live = {
            let dock = self.dock_area.read(cx);
            active_live_center_panel(dock, cx)
        };
        if let Some(live) = live {
            self.register_center_panel(live, cx);
            return;
        }
        self.reset_center_to_home(window, cx);
    }

    pub(crate) fn sync_tab_manager_from_dock(&mut self, cx: &mut Context<Self>) {
        let panels = self.tab_manager.read(cx).panels();
        self.sync_panels(panels, cx);
    }

    /// Make `panels` the live center set: re-infer specs, drop the rest, follow the dock's active tab.
    fn sync_panels(&mut self, panels: Vec<Arc<dyn PanelView>>, cx: &mut Context<Self>) {
        let dock = self.dock_area.read(cx);
        let entries: Vec<_> = panels
            .iter()
            .map(|panel| DockTab {
                panel: panel.clone(),
                inferred: infer::infer_tab_spec(panel, cx),
                fallback: infer::fallback_spec(panel, cx),
            })
            .collect();
        // The dock owns activation (clicks, ⌘W, show_home); the manager's own
        // active tab is only a fallback when the dock has no live panel.
        let active = active_live_center_panel(dock, cx)
            .map(|p| p.view())
            .or_else(|| {
                self.tab_manager
                    .read(cx)
                    .active_tab()
                    .filter(|t| panels.iter().any(|p| p.view() == t.view()))
                    .map(|t| t.view())
            });
        self.tab_manager.update(cx, |tm, ecx| {
            tm.reconcile_dock_tabs(&entries, active, ecx);
        });
        self.record_tab_navigation(cx);
        self.refresh_tab_strip_chrome(cx);
    }

    /// Whether the given center-dock panel may be closed (gpui-kit hides Close for center tabs).
    pub fn can_close_center_panel(&self, panel_id: EntityId, cx: &App) -> bool {
        if self.is_tab_pinned(panel_id, cx) {
            return false;
        }
        self.find_center_panel(panel_id, cx).is_some()
    }

    pub fn active_center_panel_id(&self, cx: &App) -> Option<EntityId> {
        let dock = self.dock_area.read(cx);
        active_live_center_panel(dock, cx).map(|p| panel_entity_id(&p))
    }
}
