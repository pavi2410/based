//! Workspace reactions to `.based/` project changes and workspace-level context updates.

use std::mem;
use std::path::PathBuf;
use std::time::Instant;

use based_project::{ProjectQuery, load_env_file};
use gpui::Context;

use crate::connection::registry::ConnectionRegistry;
use crate::connection::{
    ConnectionConfig, ConnectionEntry, ConnectionId, ConnectionOrigin, ConnectionState,
    OpenedConnection, close_any_connection, opened_into_any,
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

    /// Attach a live wizard session without writing files or closing the form.
    ///
    /// `open_catalog` queues the connection workspace (dashboard) — use on first Connect,
    /// not when replacing a live session after Edit → Save.
    pub fn attach_wizard_session(
        &mut self,
        config: ConnectionConfig,
        opened: OpenedConnection,
        session_id: ConnectionId,
        tags: Vec<String>,
        open_catalog: bool,
        cx: &mut Context<Self>,
    ) {
        let origin = if session_id.is_personal() || session_id.is_ephemeral() {
            ConnectionOrigin::Personal
        } else {
            ConnectionOrigin::Project
        };
        let mut entry = ConnectionEntry::with_origin(config, &session_id.0, tags, origin);
        entry.state = ConnectionState::Connected(opened_into_any(opened, cx));
        self.upsert_registry_entry(entry, true, cx);
        if open_catalog {
            self.connection_tree.update(cx, |tree, cx| {
                tree.queue_open_connected(&session_id, cx);
            });
        }
    }

    /// Persist succeeded but reconnect failed — drop the stale live session.
    pub fn fail_reconnect(&mut self, id: &ConnectionId, reason: String, cx: &mut Context<Self>) {
        self.registry.update(cx, |reg, cx| {
            let Some(existing) = reg.get(id, cx).cloned() else {
                return;
            };
            existing.update(cx, |e, cx| {
                if let ConnectionState::Connected(ac) =
                    mem::replace(&mut e.state, ConnectionState::Disconnected)
                {
                    close_any_connection(ac, cx);
                }
                e.state = ConnectionState::Failed {
                    reason: reason.clone(),
                    attempted_at: Instant::now(),
                };
                e.last_error = Some(reason);
                cx.notify();
            });
        });
    }

    /// Persist the wizard config to the chosen based-dir. Migrates a live unsaved session.
    /// Returns `(saved_id, reconnect)` — reconnect when a saved connection's open params changed
    /// while it was connected.
    pub fn save_wizard_connection(
        &mut self,
        config: ConnectionConfig,
        destination: ConnectionDestination,
        session_id: Option<ConnectionId>,
        tags: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Result<(ConnectionId, bool), String> {
        let based_dir = destination
            .based_dir(self.project_dir.as_deref())
            .map_err(|err| format!("{err:#}"))?;
        let origin = destination.origin();
        let editing = session_id
            .as_ref()
            .and_then(super::wizard_logic::relative_id_for_saved)
            .is_some();
        let (was_connected, old_config) = session_id
            .as_ref()
            .and_then(|id| {
                self.registry.read(cx).get(id, cx).map(|entry| {
                    let e = entry.read(cx);
                    (
                        matches!(e.state, ConnectionState::Connected(_)),
                        e.config.clone(),
                    )
                })
            })
            .unwrap_or((false, config.clone()));
        let reconnect = editing
            && super::wizard_logic::should_reconnect_after_save(
                was_connected,
                &old_config,
                &config,
            );
        let keep_id = session_id
            .as_ref()
            .and_then(super::wizard_logic::relative_id_for_saved);
        let mut entry = match persist_config_to_based_dir(&based_dir, &config, &tags, keep_id) {
            Ok(conn) => {
                let vars = load_env_file(&based_dir.join(".env")).unwrap_or_default();
                entry_from_tree(&conn, origin, &vars).unwrap_or_else(|err| {
                    log::warn!("resolve persisted connection failed: {err:#}");
                    entry_from_config(&config, &conn.id, origin, tags.clone())
                })
            }
            Err(err) => {
                log::warn!("persist connection file failed: {err:#}");
                return Err(format!("Could not save connection: {err:#}"));
            }
        };
        let saved_id = entry.id.clone();
        if let Some(from) = session_id.as_ref().filter(|from| *from != &saved_id)
            && let Some(live) = take_registry_state(&self.registry, from, cx)
        {
            entry.state = live;
        }
        let open_catalog = matches!(entry.state, ConnectionState::Connected(_));
        self.upsert_registry_entry(entry, false, cx);
        if open_catalog {
            self.connection_tree.update(cx, |tree, cx| {
                tree.queue_open_connected(&saved_id, cx);
            });
        }
        Ok((saved_id, reconnect))
    }

    fn upsert_registry_entry(
        &mut self,
        mut entry: ConnectionEntry,
        replace_live: bool,
        cx: &mut Context<Self>,
    ) {
        self.registry.update(cx, |reg, cx| {
            if let Some(existing) = reg.get(&entry.id, cx).cloned() {
                existing.update(cx, |e, cx| {
                    e.config = entry.config.clone();
                    e.origin = entry.origin;
                    e.tags = entry.tags.clone();
                    if replace_live
                        || matches!(entry.state, ConnectionState::Connected(_))
                        || matches!(e.state, ConnectionState::Disconnected)
                    {
                        let incoming =
                            mem::replace(&mut entry.state, ConnectionState::Disconnected);
                        let old = mem::replace(&mut e.state, incoming);
                        if replace_live && let ConnectionState::Connected(ac) = old {
                            close_any_connection(ac, cx);
                        }
                    }
                    e.last_error = None;
                    cx.notify();
                });
            } else {
                reg.add(entry, cx);
            }
        });
    }
}

fn take_registry_state(
    registry: &gpui::Entity<ConnectionRegistry>,
    id: &ConnectionId,
    cx: &mut Context<Workspace>,
) -> Option<ConnectionState> {
    let mut taken = None;
    registry.update(cx, |reg, cx| {
        let Some(existing) = reg.get(id, cx).cloned() else {
            return;
        };
        existing.update(cx, |e, _| {
            taken = Some(mem::replace(&mut e.state, ConnectionState::Disconnected));
        });
        reg.remove(id, cx);
    });
    taken
}

fn entry_from_config(
    config: &ConnectionConfig,
    relative_id: &str,
    origin: ConnectionOrigin,
    tags: Vec<String>,
) -> ConnectionEntry {
    let key = super::wizard_logic::saved_id_for_destination(origin, relative_id).0;
    ConnectionEntry::with_origin(config.clone(), &key, tags, origin)
}
