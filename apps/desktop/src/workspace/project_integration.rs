//! Workspace reactions to `.based/` project changes and workspace-level context updates.

use std::mem;
use std::path::PathBuf;

use based_project::ProjectQuery;
use gpui::Context;

use crate::connection::{
    ConnectionConfig, ConnectionId, ConnectionState, OpenedConnection, opened_into_any,
};
use crate::db;
use crate::project::ProjectContext;
use crate::query_store::QueryStore;
use crate::storage;

use super::Workspace;
use super::context::WorkspaceContext;
use super::project_query::{OpenQueryResult, open_project_query, tab_spec_for_query};

use super::templates;

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

    pub fn apply_workspace_context(&mut self, ctx: WorkspaceContext, cx: &mut Context<Self>) {
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
        } else {
            self.project_title = ctx.active.name.clone().into();
        }
        cx.set_global(ctx.clone());
        cx.notify();
    }

    fn persist_connection_template(
        &mut self,
        template: based_workspace::ConnectionTemplate,
        cx: &mut Context<Self>,
    ) {
        let ctx = cx.global::<WorkspaceContext>().clone();
        let store = storage::store(cx);
        let workspace_id = ctx.active.id;
        let this = cx.entity().downgrade();
        let template_for_resolve = template.clone();
        cx.spawn(async move |_, cx| {
            // Metadata SQLite uses sqlx; GPUI tasks are not on Tokio.
            let refreshed = db::run(cx, async move {
                store
                    .upsert_connection_template(workspace_id, &template)
                    .await?;
                super::context::refresh_context(store, workspace_id).await
            })
            .await;
            if let Err(err) = &refreshed {
                log::warn!("persist connection template failed: {err:#}");
                return;
            }
            cx.update(|cx| {
                let Some(this) = this.upgrade() else {
                    return;
                };
                if let Ok(ctx) = refreshed {
                    let entry =
                        templates::resolve_template_entry(&ctx.active, &template_for_resolve).ok();
                    this.update(cx, |ws, cx| {
                        ws.apply_workspace_context(ctx, cx);
                        if let Some(entry) = entry {
                            ws.registry.update(cx, |reg, cx| {
                                if reg.get(&entry.id, cx).is_none() {
                                    reg.add(entry, cx);
                                }
                            });
                        }
                    });
                }
            })
        })
        .detach();
    }

    /// Persist the wizard config, attach the live connection, and replace the wizard tab.
    pub fn finish_wizard_connect(
        &mut self,
        config: ConnectionConfig,
        opened: OpenedConnection,
        wizard_panel_id: gpui::EntityId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let ctx = cx.global::<WorkspaceContext>().clone();
        let (template, mut entry) = templates::entry_from_wizard_config(&ctx.active, &config);
        entry.state = ConnectionState::Connected(opened_into_any(opened, cx));
        let conn_id = entry.id.clone();
        self.registry.update(cx, |reg, cx| {
            if let Some(existing) = reg.get(&entry.id, cx).cloned() {
                existing.update(cx, |e, cx| {
                    e.config = entry.config.clone();
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
        self.persist_connection_template(template, cx);
        self.close_center_panel(wizard_panel_id, window, cx);
    }
}
