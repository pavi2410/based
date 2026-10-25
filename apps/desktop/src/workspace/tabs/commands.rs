use std::sync::Arc;

use gpui_kit::component::Placement;
use gpui_kit::component::dock::PanelView;
use gpui_kit::{App, AppContext, Context, EntityId, Window};

use super::spec::TabSpec;
use crate::connection::{
    AnyConnection::{MongoDB, Postgres, Redis, SQLite},
    ConnectionEntry, ConnectionId,
    ConnectionState::Connected,
};
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::postgres::query::PgQuery;
use crate::redis::console::ConsolePanel as RedisConsolePanel;
use crate::sqlite::query::SqliteQuery;
use crate::workspace::Workspace;
use crate::workspace::dock_utils::{
    activate_center_panel, active_live_center_panel, center_panel_by_id, center_tab_group_count,
    panel_entity_id, remove_presentation_panel, split_center_with_panel, tab_group_panel_ids,
    to_panel_id,
};
use crate::workspace::panels::query_editor::QueryEditorPanel;

impl Workspace {
    pub(crate) fn record_tab_navigation(&mut self, cx: &Context<Self>) {
        let dock = self.dock_area.read(cx);
        let Some(panel) = active_live_center_panel(dock, cx) else {
            return;
        };
        self.tab_navigation
            .record_activation(panel_entity_id(&panel));
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

    fn activate_center_panel_id(
        &mut self,
        panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self.find_center_panel(panel_id, cx) {
            activate_center_panel(&self.dock_area, panel, window, cx);
            cx.notify();
            return;
        }
        let panel = {
            let dock = self.dock_area.read(cx);
            center_panel_by_id(dock, panel_id, cx).map(|(_, panel, _)| panel)
        };
        if let Some(panel) = panel {
            activate_center_panel(&self.dock_area, panel, window, cx);
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
        if center_tab_group_count(dock) <= 1 {
            return false;
        }
        center_panel_by_id(dock, panel_id, cx).is_some()
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
        let removed_ids = {
            let dock = self.dock_area.read(cx);
            let Some((node, _, _)) = center_panel_by_id(dock, panel_id, cx) else {
                return;
            };
            let group_ids = tab_group_panel_ids(dock, node);
            self.tab_manager
                .read(cx)
                .panels()
                .iter()
                .filter(|p| group_ids.contains(&to_panel_id(panel_entity_id(p))))
                .map(panel_entity_id)
                .collect::<Vec<EntityId>>()
        };
        let to_remove: Vec<Arc<dyn PanelView>> = self
            .tab_manager
            .read(cx)
            .panels()
            .into_iter()
            .filter(|p| removed_ids.contains(&panel_entity_id(p)))
            .collect();
        self.dock_area.update(cx, |dock, ecx| {
            for panel in &to_remove {
                remove_presentation_panel(dock, panel, window, ecx);
            }
        });
        for id in removed_ids {
            self.unregister_center_panel(id, cx);
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

        split_center_with_panel(&self.dock_area, panel_arc.clone(), placement, window, cx);
        self.register_center_panel(panel_arc.clone(), cx);
        self.tab_manager.update(cx, |tm, ecx| {
            tm.open_or_focus(spec, panel_arc, ecx);
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
            Connected(ac) => ac.clone(),
            _ => return None,
        };
        match ac {
            SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                let panel = cx.new(|cx| {
                    QueryEditorPanel::new(
                        Arc::new(SqliteQuery { pool }),
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                Some(Arc::new(panel))
            }
            Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                let panel = cx.new(|cx| {
                    QueryEditorPanel::new(Arc::new(PgQuery { pool }), conn_id.clone(), window, cx)
                });
                Some(Arc::new(panel))
            }
            MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                let coll = db.collection("based_explorer");
                let panel = cx.new(|cx| {
                    PipelineBuilderPanel::new_with_pipeline(coll, conn_id.clone(), None, window, cx)
                });
                Some(Arc::new(panel))
            }
            Redis(conn) => {
                let conn = conn.read(cx).conn();
                let panel =
                    cx.new(|cx| RedisConsolePanel::new(conn, conn_id.clone(), None, window, cx));
                Some(Arc::new(panel))
            }
        }
    }

    fn find_connection_for_tab(
        &self,
        conn_id: &ConnectionId,
        cx: &App,
    ) -> Option<gpui_kit::Entity<ConnectionEntry>> {
        self.registry
            .read(cx)
            .connections()
            .iter()
            .find(|e| e.read(cx).id == *conn_id)
            .cloned()
    }
}
