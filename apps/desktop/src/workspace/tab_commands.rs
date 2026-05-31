use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, AppContext, Context, Entity, EntityId, Window};
use gpui_component::Placement;
use gpui_component::dock::{DockPlacement, PanelView, TabPanel};

use super::Workspace;
use super::dock_utils::{
    activate_center_panel, active_center_tab_panel, active_live_center_panel, center_panel_by_id,
    center_tab_panel_count,
};
use super::tab_spec::TabSpec;

pub(crate) fn panel_index_in_strip(
    center: &gpui_component::dock::DockItem,
    panel_id: EntityId,
    cx: &App,
) -> Option<(Entity<TabPanel>, usize, Arc<dyn PanelView>)> {
    super::dock_utils::center_panel_by_id(center, panel_id, cx)
        .map(|(tab_panel, panel, ix)| (tab_panel, ix, panel))
}

impl Workspace {
    pub(crate) fn record_tab_navigation(&mut self, cx: &Context<Self>) {
        let dock = self.dock_area.read(cx);
        let Some(panel) = active_live_center_panel(dock.center(), cx) else {
            return;
        };
        self.tab_navigation.record_activation(panel.panel_id(cx));
    }

    pub fn go_back_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel_id) = self.tab_navigation.go_back() else {
            return;
        };
        self.activate_center_panel_id(panel_id, window, cx);
    }

    pub fn go_forward_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel_id) = self.tab_navigation.go_forward() else {
            return;
        };
        self.activate_center_panel_id(panel_id, window, cx);
    }

    pub fn tab_navigation_can_go_back(&self) -> bool {
        self.tab_navigation.can_go_back()
    }

    pub fn tab_navigation_can_go_forward(&self) -> bool {
        self.tab_navigation.can_go_forward()
    }

    fn activate_center_panel_id(
        &mut self,
        panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let center = self.dock_area.read(cx).center().clone();
        if let Some(panel) = self.find_center_panel(panel_id, cx) {
            activate_center_panel(&center, panel, window, cx);
            cx.notify();
        } else if let Some((tab_panel, panel, _)) =
            super::dock_utils::center_panel_by_id(&center, panel_id, cx)
        {
            if tab_panel
                .read(cx)
                .active_panel(cx)
                .is_some_and(|p| p.panel_id(cx) == panel_id)
            {
                cx.notify();
                return;
            }
            tab_panel.update(cx, |tp, cx| {
                tp.remove_panel(panel.clone(), window, cx);
                tp.add_panel(panel, window, cx);
            });
            cx.notify();
        }
    }

    pub fn toggle_pin_tab(&mut self, panel_id: EntityId, cx: &mut Context<Self>) {
        self.tab_manager.update(cx, |tm, ecx| {
            if let Some(tab) = tm.tab_for_panel_id_mut(panel_id) {
                tab.pinned = !tab.pinned;
                ecx.notify();
            }
        });
        self.refresh_tab_strip_chrome(cx);
        self.save_session(cx);
    }

    pub(crate) fn refresh_tab_strip_chrome(&self, cx: &mut App) {
        self.dock_area.update(cx, |_, cx| cx.notify());
    }

    pub fn is_tab_pinned(&self, panel_id: EntityId, cx: &App) -> bool {
        self.tab_manager
            .read(cx)
            .tab_for_panel_id(panel_id)
            .is_some_and(|t| t.pinned)
    }

    pub fn open_new_query_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn_id) = self.focused_conn_id(cx) else {
            return;
        };
        self.dispatch_open_tab(TabSpec::blank_query_editor(conn_id), window, cx);
    }

    /// True when the center area has multiple split panes and this tab's pane can be removed.
    pub fn can_close_center_pane(&self, panel_id: EntityId, cx: &App) -> bool {
        let dock = self.dock_area.read(cx);
        let center = dock.center();
        if center_tab_panel_count(center) <= 1 {
            return false;
        }
        center_panel_by_id(center, panel_id, cx).is_some()
    }

    /// Remove the entire split pane (tab group) containing this tab, including all tabs in it.
    pub fn close_center_pane(
        &mut self,
        panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_close_center_pane(panel_id, cx) {
            return;
        }
        let dock = self.dock_area.read(cx);
        let center = dock.center().clone();
        let Some((tab_panel, _, _)) = center_panel_by_id(&center, panel_id, cx) else {
            return;
        };
        let tab_panel_id = tab_panel.entity_id();
        let tab_panel_view: Arc<dyn PanelView> = Arc::new(tab_panel);
        let removed_ids: Vec<EntityId> = self
            .center_panels
            .iter()
            .filter(|p| {
                center_panel_by_id(&center, p.panel_id(cx), cx)
                    .is_some_and(|(tp, _, _)| tp.entity_id() == tab_panel_id)
            })
            .map(|p| p.panel_id(cx))
            .collect();
        self.dock_area.update(cx, |dock, ecx| {
            dock.remove_panel(tab_panel_view, DockPlacement::Center, window, ecx);
        });
        for id in removed_ids {
            self.unregister_center_panel(id);
        }
        self.sync_tab_manager_from_dock(cx);
        self.ensure_home_tab(window, cx);
        cx.notify();
    }

    pub fn split_center_pane(
        &mut self,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conn_id) = self.focused_conn_id(cx) else {
            return;
        };
        let spec = TabSpec::blank_query_editor(conn_id);
        let Some(panel_arc) = self.build_query_panel_for_split(&spec, window, cx) else {
            return;
        };

        let dock = self.dock_area.read(cx);
        let Some(tab_panel) = active_center_tab_panel(dock.center()) else {
            return;
        };
        tab_panel.update(cx, |tp, ecx| {
            tp.add_panel_at(panel_arc.clone(), placement, None, window, ecx);
        });
        let view: gpui::AnyView = panel_arc.as_ref().into();
        self.register_center_panel(panel_arc, cx);
        self.tab_manager.update(cx, |tm, ecx| {
            tm.open_or_focus(spec, view, ecx);
        });
        cx.notify();
    }

    fn build_query_panel_for_split(
        &self,
        spec: &TabSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<dyn PanelView>> {
        let TabSpec::QueryEditor { conn_id, .. } = spec else {
            return None;
        };
        let ent = self.find_connection_for_tab(conn_id, cx)?;
        let ac = match &ent.read(cx).state {
            crate::connection::ConnectionState::Connected(ac) => ac.clone(),
            _ => return None,
        };
        match ac {
            crate::connection::AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                let panel = cx.new(|cx| {
                    crate::sqlite::query_editor::QueryEditorPanel::new(
                        pool,
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                Some(Arc::new(panel))
            }
            crate::connection::AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                let panel = cx.new(|cx| {
                    crate::postgres::query_editor::QueryEditorPanel::new(
                        pool,
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                Some(Arc::new(panel))
            }
            crate::connection::AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                let coll = db.collection("based_explorer");
                let panel = cx.new(|cx| {
                    crate::mongodb::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
                        coll,
                        conn_id.clone(),
                        None,
                        window,
                        cx,
                    )
                });
                Some(Arc::new(panel))
            }
        }
    }

    fn find_connection_for_tab(
        &self,
        conn_id: &crate::connection::ConnectionId,
        cx: &App,
    ) -> Option<gpui::Entity<crate::connection::ConnectionEntry>> {
        self.registry
            .read(cx)
            .connections()
            .iter()
            .find(|e| e.read(cx).id == *conn_id)
            .cloned()
    }

    pub fn close_all_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_matching_tabs(window, cx, |_panel_id, tab, closable| {
            closable && !tab.pinned
        });
    }

    pub fn close_clean_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_matching_tabs(window, cx, |_panel_id, tab, closable| {
            closable && !tab.pinned && !tab.dirty
        });
    }

    pub fn close_other_tabs(
        &mut self,
        keep_panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_matching_tabs(window, cx, |panel_id, tab, closable| {
            panel_id != keep_panel_id && closable && !tab.pinned
        });
    }

    pub fn close_tabs_to_left(
        &mut self,
        anchor_panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index_by_panel = self.center_tab_index_map(cx);
        let Some(anchor_ix) = index_by_panel.get(&anchor_panel_id).copied() else {
            return;
        };
        self.close_matching_tabs(window, cx, |panel_id, tab, closable| {
            index_by_panel
                .get(&panel_id)
                .is_some_and(|ix| *ix < anchor_ix)
                && closable
                && !tab.pinned
        });
    }

    pub fn close_tabs_to_right(
        &mut self,
        anchor_panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index_by_panel = self.center_tab_index_map(cx);
        let Some(anchor_ix) = index_by_panel.get(&anchor_panel_id).copied() else {
            return;
        };
        self.close_matching_tabs(window, cx, |panel_id, tab, closable| {
            index_by_panel
                .get(&panel_id)
                .is_some_and(|ix| *ix > anchor_ix)
                && closable
                && !tab.pinned
        });
    }

    fn center_tab_index_map(&self, cx: &App) -> HashMap<EntityId, usize> {
        self.center_panels
            .iter()
            .enumerate()
            .map(|(ix, p)| (p.panel_id(cx), ix))
            .collect()
    }

    fn close_matching_tabs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mut pred: impl FnMut(EntityId, &super::tab_manager::Tab, bool) -> bool,
    ) {
        let candidates: Vec<(EntityId, Arc<dyn PanelView>)> = self
            .center_panels
            .iter()
            .filter_map(|p| {
                let panel_id = p.panel_id(cx);
                let tab = self.tab_manager.read(cx).tab_for_panel_id(panel_id)?;
                if pred(panel_id, tab, true) {
                    Some((panel_id, p.clone()))
                } else {
                    None
                }
            })
            .collect();

        for (panel_id, panel) in candidates {
            if !self.can_close_center_panel(panel_id, cx) {
                continue;
            }
            self.dock_area.update(cx, |dock, ecx| {
                dock.remove_panel(panel, DockPlacement::Center, window, ecx);
            });
            self.unregister_center_panel(panel_id);
        }
        self.sync_tab_manager_from_dock(cx);
        self.ensure_home_tab(window, cx);
        cx.notify();
    }
}
