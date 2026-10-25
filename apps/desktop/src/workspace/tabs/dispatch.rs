use std::sync::Arc;

use gpui_kit::component::dock::PanelView;
use gpui_kit::{App, Context, Entity, Window, prelude::*};

use crate::connection::{AnyConnection, ConnectionEntry, ConnectionId, ConnectionState};

use super::label::tab_label_for_spec;
use super::spec::TabSpec;
use crate::mongodb::tab_dispatch as mongo_tab_dispatch;
use crate::postgres::tab_dispatch as pg_tab_dispatch;
use crate::redis::tab_dispatch as redis_tab_dispatch;
use crate::sqlite::tab_dispatch as sqlite_tab_dispatch;
use crate::workspace::Workspace;
use crate::workspace::dock_utils::{activate_center_panel, add_center_panel_view};
use crate::workspace::panels::object_info::{ConnectionDashboardPanel, ObjectInfoPanel};
use crate::workspace::panels::release_notes::ReleaseNotesPanel;

impl Workspace {
    fn find_connection(&self, id: &ConnectionId, cx: &App) -> Option<Entity<ConnectionEntry>> {
        self.registry
            .read(cx)
            .connections()
            .iter()
            .find(|e| e.read(cx).id == *id)
            .cloned()
    }

    pub fn dispatch_open_tab(
        &mut self,
        spec: TabSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // Re-opening an existing tab focuses it. Dashboard focuses its connection first.
        if !matches!(
            spec,
            TabSpec::Home
                | TabSpec::Builtin { .. }
                | TabSpec::QueryEditor { .. }
                | TabSpec::Dashboard { .. }
        ) && self.focus_existing_tab(&spec, window, cx)
        {
            return true;
        }

        // ── Connection-independent tabs ───────────────────────────────────────────
        match &spec {
            TabSpec::Home => {
                self.show_home(window, cx);
                return true;
            }
            TabSpec::ReleaseNotes { version } => {
                let label = tab_label_for_spec(&spec, false);
                let v = version.clone();
                let panel = cx.new(|cx| ReleaseNotesPanel::new(v, window, cx));
                panel.update(cx, |p, _| p.tab_label = label);
                self.dock_add_and_register_tab(spec, Arc::new(panel), window, cx);
                return true;
            }
            TabSpec::ObjectInfo {
                object_name,
                kind_label,
                ..
            } => {
                let label = tab_label_for_spec(&spec, false);
                let on = object_name.clone();
                let kl = kind_label.clone();
                let panel = cx.new(|cx| ObjectInfoPanel::new(on, kl, window, cx));
                panel.update(cx, |p, _| p.tab_label = label);
                self.dock_add_and_register_tab(spec, Arc::new(panel), window, cx);
                return true;
            }
            TabSpec::Dashboard { conn_id } => {
                self.connection_tree.update(cx, |tree, ecx| {
                    tree.focus_connection_by_id(conn_id, ecx);
                });
                if self.focus_existing_tab(&spec, window, cx) {
                    return true;
                }
                let Some(ent) = self.find_connection(conn_id, cx) else {
                    return false;
                };
                let panel = cx.new(|cx| ConnectionDashboardPanel::new(ent, window, cx));
                self.dock_add_and_register_tab(spec, Arc::new(panel), window, cx);
                return true;
            }
            TabSpec::Builtin { .. } => return true,
            _ => {}
        }

        // ── Connection-scoped tabs ────────────────────────────────────────────────
        let Some(conn_id) = spec.conn_id().cloned() else {
            return false;
        };
        let Some(ent) = self.find_connection(&conn_id, cx) else {
            return false;
        };
        let ac = match &ent.read(cx).state {
            ConnectionState::Connected(ac) => ac.clone(),
            _ => return false,
        };

        // Each engine's tab_dispatch::build_panel handles its own tab types.
        // To add a new engine: add one arm here + create its tab_dispatch.rs.
        // To add a new tab type for an existing engine: edit that engine's tab_dispatch.rs only.
        let panel = match ac {
            AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                pg_tab_dispatch::build_panel(&spec, pool, &conn_id, window, cx)
            }
            AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                sqlite_tab_dispatch::build_panel(&spec, pool, &conn_id, window, cx)
            }
            AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                mongo_tab_dispatch::build_panel(&spec, db, &conn_id, window, cx)
            }
            AnyConnection::Redis(conn) => {
                let conn = conn.read(cx).conn();
                redis_tab_dispatch::build_panel(&spec, conn, &conn_id, window, cx)
            }
        };

        if let Some(panel) = panel {
            self.dock_add_and_register_tab(spec, panel, window, cx);
            true
        } else {
            true
        }
    }

    /// Activate the open tab for `spec`, if any.
    pub(crate) fn focus_existing_tab(
        &mut self,
        spec: &TabSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel_id) = self
            .tab_manager
            .read(cx)
            .tabs
            .iter()
            .find(|t| t.spec == *spec)
            .map(|t| t.view().entity_id())
        else {
            return false;
        };
        if let Some(panel) = self.find_center_panel(panel_id, cx) {
            activate_center_panel(&self.dock_area, panel, window, cx);
            cx.notify();
        }
        true
    }

    fn dock_add_and_register_tab(
        &mut self,
        spec: TabSpec,
        panel: Arc<dyn PanelView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        add_center_panel_view(&self.dock_area, panel.clone(), window, cx);
        self.register_center_panel(panel.clone(), cx);
        self.tab_manager.update(cx, |tm, ecx| {
            tm.open_or_focus(spec, panel, ecx);
        });
        cx.notify();
    }
}
