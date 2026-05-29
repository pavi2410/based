//! Active workspace model and top-bar context (workspace + environment).

use std::sync::Arc;

use anyhow::Result;
use based_storage::{MetadataStore, WorkspaceSummary};
use based_workspace::WorkspaceModel;
use gpui::{App, Global};

use crate::storage;

pub const NO_ENVIRONMENT_LABEL: &str = "No Environment";

#[derive(Clone)]
pub struct WorkspaceContext {
    pub active: WorkspaceModel,
    pub summaries: Vec<WorkspaceSummary>,
}

impl Global for WorkspaceContext {}

impl WorkspaceContext {
    pub fn load_initial(cx: &App) -> Result<Self> {
        let store = storage::store(cx);
        let handle = gpui_tokio::Tokio::handle(cx);
        handle.block_on(async move {
            let active = store.ensure_default_workspace().await?;
            let summaries = store.list_workspaces().await?;
            Ok(Self { active, summaries })
        })
    }

    pub fn workspace_options(&self) -> Vec<String> {
        self.summaries
            .iter()
            .map(|s| s.name.clone())
            .collect()
    }

    pub fn environment_options(&self) -> Vec<String> {
        let mut opts = vec![NO_ENVIRONMENT_LABEL.to_string()];
        for env in &self.active.environments {
            opts.push(env.name.clone());
        }
        opts
    }

    pub fn active_workspace_index(&self) -> usize {
        self.summaries
            .iter()
            .position(|s| s.id == self.active.id)
            .unwrap_or(0)
    }

    pub fn active_environment_index(&self) -> usize {
        match self.active.active_environment_id {
            None => 0,
            Some(id) => self
                .active
                .environments
                .iter()
                .position(|e| e.id == id)
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }
}

use anyhow::Context as _;

pub async fn refresh_context(
    store: Arc<MetadataStore>,
    workspace_id: uuid::Uuid,
) -> Result<WorkspaceContext> {
    let active = store
        .get_workspace(workspace_id)
        .await?
        .context("workspace missing")?;
    let summaries = store.list_workspaces().await?;
    Ok(WorkspaceContext { active, summaries })
}

pub async fn create_workspace(store: Arc<MetadataStore>, name: &str) -> Result<WorkspaceContext> {
    let ws = store.create_workspace(name).await?;
    store.set_active_workspace_id(Some(ws.id)).await?;
    refresh_context(store, ws.id).await
}

pub async fn switch_workspace(
    store: Arc<MetadataStore>,
    workspace_id: uuid::Uuid,
) -> Result<WorkspaceContext> {
    store.set_active_workspace_id(Some(workspace_id)).await?;
    refresh_context(store, workspace_id).await
}

pub async fn set_active_environment(
    store: Arc<MetadataStore>,
    ctx: &WorkspaceContext,
    index: usize,
) -> Result<WorkspaceContext> {
    let env_id = if index == 0 {
        None
    } else {
        Some(
            ctx.active
                .environments
                .get(index - 1)
                .context("environment index out of range")?
                .id,
        )
    };
    store
        .set_active_environment(ctx.active.id, env_id)
        .await?;
    refresh_context(store, ctx.active.id).await
}

pub async fn create_environment(
    store: Arc<MetadataStore>,
    ctx: &WorkspaceContext,
    name: &str,
) -> Result<WorkspaceContext> {
    store.create_environment(ctx.active.id, name).await?;
    refresh_context(store, ctx.active.id).await
}
