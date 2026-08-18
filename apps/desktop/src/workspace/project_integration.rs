//! Workspace reactions to `.based/` project changes and workspace-level context updates.

use std::mem;
use std::path::PathBuf;

use based_project::{ProjectQuery, load_env_file, slug_from_label};
use gpui::Context;

use crate::connection::{
    ConnectionConfig, ConnectionEntry, ConnectionId, ConnectionOrigin, ConnectionState,
    OpenedConnection, opened_into_any,
};
use crate::project::ProjectContext;
use crate::project::loader::entry_from_tree;
use crate::query_store::QueryStore;

use super::Workspace;
use super::connection_destination::ConnectionDestination;
use super::connection_persist::persist_config_to_based_dir;
use super::context::WorkspaceContext;
use super::project_query::{OpenQueryResult, open_project_query, tab_spec_for_query};

impl Workspace {
    pub fn set_pending_target_pick(&mut self, query: ProjectQuery, candidates: Vec<ConnectionId>) {
        self.pending_target_pick = Some((query, candidates));
    }

    pub fn resolve_pending_target(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        if let Some((query, _)) = self.pending_target_pick.take() {
            self.pending_open_tab = Some(tab_spec_for_query(&query, conn_id));
            cx.notify();
        }
    }

    pub fn cancel_pending_target_pick(&mut self, cx: &mut Context<Self>) {
        if self.pending_target_pick.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn open_project_query_by_path(&mut self, path: &str, cx: &mut Context<Self>) {
        let store = cx.global::<QueryStore>();
        let Some(query) = store.project_queries().iter().find(|q| q.path == path) else {
            log::warn!("project query not found: {path}");
            return;
        };
        let focused = self.focused_conn_id(cx);
        match open_project_query(query, self.registry.read(cx), cx, focused.as_ref()) {
            OpenQueryResult::Open(spec) => {
                self.pending_open_tab = Some(spec);
            }
            OpenQueryResult::PickConnection { candidates, .. } => {
                self.pending_target_pick = Some((query.clone(), candidates));
            }
            OpenQueryResult::Error(msg) => log::warn!("{msg}"),
        }
    }

    pub fn sync_project_context(&mut self, cx: &mut Context<Self>) {
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
            cx.notify();
        }
    }

    pub fn apply_opened_project(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.project_dir = Some(root);
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
        }
        self.connection_tree.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn apply_closed_project(&mut self, cx: &mut Context<Self>) {
        self.project_dir = None;
        self.project_title = "No project".into();
        self.connection_tree.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn apply_workspace_context(&mut self, ctx: WorkspaceContext, cx: &mut Context<Self>) {
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
        } else {
            self.project_title = ctx.active.name.clone().into();
        }
        cx.set_global(ctx.clone());
        cx.notify();
    }

    /// Persist the wizard config as `.toml` in the chosen based-dir, attach the
    /// live connection, and replace the wizard tab.
    pub fn finish_wizard_connect(
        &mut self,
        config: ConnectionConfig,
        opened: OpenedConnection,
        destination: ConnectionDestination,
        wizard_panel_id: gpui::EntityId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let based_dir = match destination.based_dir(self.project_dir.as_deref()) {
            Ok(dir) => dir,
            Err(err) => {
                log::warn!("connection destination: {err:#}");
                return;
            }
        };
        let origin = destination.origin();
        let mut entry = match persist_config_to_based_dir(&based_dir, &config) {
            Ok(conn) => {
                let vars = load_env_file(&based_dir.join(".env")).unwrap_or_default();
                entry_from_tree(&conn, origin, &vars).unwrap_or_else(|err| {
                    log::warn!("resolve persisted connection failed: {err:#}");
                    entry_from_config(&config, &conn.id, origin)
                })
            }
            Err(err) => {
                log::warn!("persist connection file failed: {err:#}");
                let slug = slug_from_label(config.label());
                entry_from_config(&config, &slug, origin)
            }
        };
        entry.state = ConnectionState::Connected(opened_into_any(opened, cx));
        let conn_id = entry.id.clone();
        self.registry.update(cx, |reg, cx| {
            if let Some(existing) = reg.get(&entry.id, cx).cloned() {
                existing.update(cx, |e, cx| {
                    e.config = entry.config.clone();
                    e.origin = entry.origin;
                    e.state = mem::replace(&mut entry.state, ConnectionState::Disconnected);
                    e.last_error = None;
                    cx.notify();
                });
            } else {
                reg.add(entry, cx);
            }
        });
        self.connection_tree.update(cx, |tree, cx| {
            tree.queue_open_connected(&conn_id, cx);
        });
        self.close_center_panel(wizard_panel_id, window, cx);
    }
}

fn entry_from_config(
    config: &ConnectionConfig,
    relative_id: &str,
    origin: ConnectionOrigin,
) -> ConnectionEntry {
    let key = match origin {
        ConnectionOrigin::Personal => ConnectionId::personal(relative_id).0,
        ConnectionOrigin::Project => relative_id.to_string(),
    };
    ConnectionEntry::with_origin(config.clone(), &key, vec![], origin)
}
