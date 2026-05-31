//! Center dock panel registry — tracks live panels, manages Home tab, and keeps TabManager in sync.

use std::sync::Arc;

use gpui::{App, Context, EntityId, Focusable, Window, prelude::*};
use gpui_component::dock::{DockItem, DockPlacement, PanelView};

use super::Workspace;
use super::dock_utils::{activate_center_panel, active_live_center_panel, wrap_center_root};

impl Workspace {
    /// Open the Postgres connection wizard in a new center tab.
    pub fn open_postgres_wizard_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.new(|cx| crate::postgres::wizard::ConnectionWizardPanel::new(window, cx));
        let arc: Arc<dyn PanelView> = Arc::new(panel);
        self.dock_area.update(cx, |dock, ecx| {
            dock.add_panel(arc.clone(), DockPlacement::Center, None, window, ecx);
        });
        self.register_center_panel(arc, cx);
        cx.notify();
    }

    pub(crate) fn register_center_panel(
        &mut self,
        panel: Arc<dyn PanelView>,
        cx: &mut Context<Self>,
    ) {
        let id = panel.view().entity_id();
        if !self
            .center_panels
            .iter()
            .any(|p| p.view().entity_id() == id)
        {
            self.center_panels.push(panel);
            self.sync_tab_manager_from_dock(cx);
        }
    }

    pub(crate) fn replace_center_panels(
        &mut self,
        panels: Vec<Arc<dyn PanelView>>,
        cx: &mut Context<Self>,
    ) {
        self.center_panels = panels;
        self.sync_tab_manager_from_dock(cx);
    }

    pub(crate) fn unregister_center_panel(&mut self, panel_id: EntityId) {
        self.center_panels
            .retain(|p| p.view().entity_id() != panel_id);
    }

    pub(crate) fn find_center_panel(
        &self,
        panel_id: EntityId,
        cx: &App,
    ) -> Option<Arc<dyn PanelView>> {
        self.center_panels
            .iter()
            .find(|p| p.panel_id(cx) == panel_id)
            .cloned()
    }

    /// Rebuild the center dock with a single Home tab (used after the last tab closes).
    fn reset_center_to_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = self.dock_area.downgrade();
        let home_arc: Arc<dyn PanelView> = Arc::new(self.home_panel.clone());
        let tabs = DockItem::tab(self.home_panel.clone(), &weak, window, cx);
        let center = wrap_center_root(tabs, &weak, window, cx);
        self.dock_area.update(cx, |dock, ecx| {
            dock.set_center(center, window, ecx);
        });
        self.replace_center_panels(vec![home_arc], cx);
        self.home_panel.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Focus the Home tab, re-adding it to the center strip if it was replaced.
    pub fn show_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home_id = self.home_panel.entity_id();
        let home_present = self.center_panels.iter().any(|p| p.panel_id(cx) == home_id);

        if home_present {
            let home = self
                .find_center_panel(home_id, cx)
                .unwrap_or_else(|| Arc::new(self.home_panel.clone()) as Arc<dyn PanelView>);
            let center = self.dock_area.read(cx).center().clone();
            activate_center_panel(&center, home, window, cx);
        } else {
            let dock = self.dock_area.read(cx);
            let has_other_live = active_live_center_panel(dock.center(), cx)
                .is_some_and(|p| p.panel_id(cx) != home_id);
            if has_other_live {
                let panel: Arc<dyn PanelView> = Arc::new(self.home_panel.clone());
                self.dock_area.update(cx, |dock, ecx| {
                    dock.add_panel(panel.clone(), DockPlacement::Center, None, window, ecx);
                });
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
        let dock = self.dock_area.read(cx);
        if let Some(live) = active_live_center_panel(dock.center(), cx) {
            if !self
                .center_panels
                .iter()
                .any(|p| p.panel_id(cx) == live.panel_id(cx))
            {
                self.register_center_panel(live, cx);
            }
            return;
        }
        if !self.center_panels.is_empty() {
            self.center_panels.clear();
        }
        self.reset_center_to_home(window, cx);
    }

    pub(crate) fn sync_tab_manager_from_dock(&mut self, cx: &mut Context<Self>) {
        let dock = self.dock_area.read(cx);
        let entries: Vec<_> = self
            .center_panels
            .iter()
            .map(|panel| {
                let view = panel.view();
                let spec = super::tabs::infer::infer_tab_spec(panel, cx);
                (view, spec)
            })
            .collect();
        let active = self
            .tab_manager
            .read(cx)
            .active_tab()
            .filter(|t| self.center_panels.iter().any(|p| p.view() == t.view))
            .map(|t| t.view.clone())
            .or_else(|| active_live_center_panel(dock.center(), cx).map(|p| p.view()));
        self.tab_manager.update(cx, |tm, ecx| {
            tm.reconcile_dock_tabs(&entries, active, ecx);
        });
        self.record_tab_navigation(cx);
        self.refresh_tab_strip_chrome(cx);
    }

    /// Number of live center tabs.
    pub fn center_tab_count(&self, _cx: &App) -> usize {
        self.center_panels.len()
    }

    /// Whether the given center-dock panel may be closed (gpui-component hides Close for center tabs).
    pub fn can_close_center_panel(&self, panel_id: EntityId, cx: &App) -> bool {
        if self.is_tab_pinned(panel_id, cx) {
            return false;
        }
        self.center_panels
            .iter()
            .any(|p| p.panel_id(cx) == panel_id)
    }

    /// Close a specific panel in the center tab strip (tab ⋮ menu).
    pub fn close_center_panel(
        &mut self,
        panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_close_center_panel(panel_id, cx) {
            return;
        }
        let Some(panel) = self.find_center_panel(panel_id, cx) else {
            return;
        };
        self.dock_area.update(cx, |dock, ecx| {
            dock.remove_panel(panel, DockPlacement::Center, window, ecx);
        });
        self.unregister_center_panel(panel_id);
        self.sync_tab_manager_from_dock(cx);
        self.ensure_home_tab(window, cx);
    }

    /// Close the active center tab (⌘W / CloseTab).
    pub fn close_active_center_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dock = self.dock_area.read(cx);
        let active_id = active_live_center_panel(dock.center(), cx)
            .map(|p| p.panel_id(cx))
            .or_else(|| {
                self.tab_manager
                    .read(cx)
                    .active_tab()
                    .map(|t| t.view.entity_id())
            });
        let Some(panel_id) = active_id else {
            self.ensure_home_tab(window, cx);
            return;
        };
        self.close_center_panel(panel_id, window, cx);
    }

    pub fn active_center_panel_id(&self, cx: &App) -> Option<EntityId> {
        let dock = self.dock_area.read(cx);
        active_live_center_panel(dock.center(), cx).map(|p| p.panel_id(cx))
    }
}
