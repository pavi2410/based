//! Active workspace model and top-bar context (workspace + environment).

use std::sync::Arc;

use anyhow::{Context as _, Result};
use based_storage::{MetadataStore, WorkspaceSummary};
use based_workspace::WorkspaceModel;
use gpui_kit::{App, Global};

use crate::db::Tokio;
use crate::storage;

#[derive(Clone)]
pub struct WorkspaceContext {
    pub active: WorkspaceModel,
    pub summaries: Vec<WorkspaceSummary>,
}

impl Global for WorkspaceContext {}

impl WorkspaceContext {
    pub fn load_initial(cx: &App) -> Result<Self> {
        let store = storage::try_store(cx).context("metadata store not initialized")?;
        let handle = Tokio::handle(cx);
        handle.block_on(async move {
            let active = store.ensure_default_workspace().await?;
            let summaries = store.list_workspaces().await?;
            Ok(Self { active, summaries })
        })
    }

    pub fn workspace_options(&self) -> Vec<String> {
        self.summaries.iter().map(|s| s.name.clone()).collect()
    }
}

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
