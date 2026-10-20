//! Workspaces and environments.

use anyhow::{Context, Result, bail};
use based_workspace::{Environment, WorkspaceModel};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{MetadataStore, WorkspaceSummary, format_rfc3339, parse_rfc3339, parse_uuid};
use crate::session_keys::{ACTIVE_ENVIRONMENT_ID, ACTIVE_WORKSPACE_ID};

impl MetadataStore {
    pub async fn active_workspace_id(&self) -> Result<Option<Uuid>> {
        self.get_session_json(ACTIVE_WORKSPACE_ID).await
    }

    pub async fn set_active_workspace_id(&self, id: Option<Uuid>) -> Result<()> {
        match id {
            Some(id) => self.set_session_json(ACTIVE_WORKSPACE_ID, &id).await,
            None => self.delete_session_key(ACTIVE_WORKSPACE_ID).await,
        }
    }

    // ── Workspace CRUD ──────────────────────────────────────────────────────

    pub async fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, name, updated_at FROM workspaces ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .context("list workspaces")?;

        rows.into_iter()
            .map(|(id, name, updated_at)| {
                Ok(WorkspaceSummary {
                    id: parse_uuid(&id)?,
                    name,
                    updated_at: parse_rfc3339(&updated_at)?,
                })
            })
            .collect()
    }

    pub async fn get_workspace(&self, id: Uuid) -> Result<Option<WorkspaceModel>> {
        let id_str = id.to_string();
        let row: Option<(String, String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, created_at, updated_at, active_environment_id
             FROM workspaces WHERE id = ?",
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await
        .context("load workspace row")?;

        let Some((id, name, created_at, updated_at, active_environment_id)) = row else {
            return Ok(None);
        };

        let active_environment_id = active_environment_id.map(|s| parse_uuid(&s)).transpose()?;

        let environments = self.load_environments(&id_str).await?;
        let connection_templates = self.load_templates(&id_str).await?;
        let collections = self.load_collections(&id_str).await?;
        let loose_queries = self.load_loose_queries(&id_str).await?;

        Ok(Some(WorkspaceModel {
            id: parse_uuid(&id)?,
            name,
            created_at: parse_rfc3339(&created_at)?,
            updated_at: parse_rfc3339(&updated_at)?,
            active_environment_id,
            environments,
            connection_templates,
            collections,
            loose_queries,
        }))
    }

    pub async fn create_workspace(&self, name: &str) -> Result<WorkspaceModel> {
        let now = OffsetDateTime::now_utc();
        let ws = WorkspaceModel {
            id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: now,
            updated_at: now,
            active_environment_id: None,
            environments: Vec::new(),
            connection_templates: Vec::new(),
            collections: Vec::new(),
            loose_queries: Vec::new(),
        };
        self.insert_workspace_row(&ws).await?;
        Ok(ws)
    }

    pub async fn ensure_default_workspace(&self) -> Result<WorkspaceModel> {
        if let Some(id) = self.active_workspace_id().await?
            && let Some(ws) = self.get_workspace(id).await?
        {
            return Ok(ws);
        }

        let workspaces = self.list_workspaces().await?;
        if let Some(summary) = workspaces.first() {
            let ws = self
                .get_workspace(summary.id)
                .await?
                .context("workspace summary missing row")?;
            self.set_active_workspace_id(Some(ws.id)).await?;
            return Ok(ws);
        }

        let ws = self.create_workspace("Default").await?;
        self.set_active_workspace_id(Some(ws.id)).await?;
        Ok(ws)
    }

    pub async fn rename_workspace(&self, id: Uuid, name: &str) -> Result<()> {
        let now = format_rfc3339(OffsetDateTime::now_utc());
        let rows = sqlx::query("UPDATE workspaces SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(&now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .context("rename workspace")?
            .rows_affected();
        if rows == 0 {
            bail!("workspace not found");
        }
        Ok(())
    }

    pub async fn delete_workspace(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM workspaces WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .context("delete workspace")?;
        Ok(())
    }

    pub async fn save_workspace(&self, workspace: &WorkspaceModel) -> Result<()> {
        let mut tx = self.pool.begin().await.context("begin save workspace tx")?;
        let id = workspace.id.to_string();
        let now = format_rfc3339(OffsetDateTime::now_utc());
        sqlx::query(
            "UPDATE workspaces SET name = ?, updated_at = ?, active_environment_id = ?
             WHERE id = ?",
        )
        .bind(&workspace.name)
        .bind(&now)
        .bind(workspace.active_environment_id.map(|u| u.to_string()))
        .bind(&id)
        .execute(&mut *tx)
        .await
        .context("update workspace row")?;

        sqlx::query("DELETE FROM environments WHERE workspace_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .context("clear environments")?;
        for env in &workspace.environments {
            let vars = serde_json::to_string(&env.variables).context("serialize env vars")?;
            sqlx::query(
                "INSERT INTO environments (id, workspace_id, name, variables_json)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(env.id.to_string())
            .bind(&id)
            .bind(&env.name)
            .bind(vars)
            .execute(&mut *tx)
            .await
            .context("insert environment")?;
        }

        sqlx::query("DELETE FROM connection_templates WHERE workspace_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .context("clear templates")?;
        for (idx, template) in workspace.connection_templates.iter().enumerate() {
            self.insert_template(&mut tx, &id, template, idx as i64)
                .await?;
        }

        sqlx::query("DELETE FROM collections WHERE workspace_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .context("clear collections")?;
        for (idx, collection) in workspace.collections.iter().enumerate() {
            let coll_id = collection.id.to_string();
            sqlx::query(
                "INSERT INTO collections (id, workspace_id, name, sort_order)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&coll_id)
            .bind(&id)
            .bind(&collection.name)
            .bind(idx as i64)
            .execute(&mut *tx)
            .await
            .context("insert collection")?;

            for (qidx, query) in collection.queries.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO queries
                     (id, workspace_id, collection_id, name, sql_text, connection_template_id, sort_order)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(query.id.to_string())
                .bind(&id)
                .bind(&coll_id)
                .bind(&query.name)
                .bind(&query.sql)
                .bind(query.connection_template_id.map(|u| u.to_string()))
                .bind(qidx as i64)
                .execute(&mut *tx)
                .await
                .context("insert collection query")?;
            }
        }

        sqlx::query("DELETE FROM queries WHERE workspace_id = ? AND collection_id IS NULL")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .context("clear loose queries")?;
        for (idx, query) in workspace.loose_queries.iter().enumerate() {
            sqlx::query(
                "INSERT INTO queries
                 (id, workspace_id, collection_id, name, sql_text, connection_template_id, sort_order)
                 VALUES (?, ?, NULL, ?, ?, ?, ?)",
            )
            .bind(query.id.to_string())
            .bind(&id)
            .bind(&query.name)
            .bind(&query.sql)
            .bind(query.connection_template_id.map(|u| u.to_string()))
            .bind(idx as i64)
            .execute(&mut *tx)
            .await
            .context("insert loose query")?;
        }

        tx.commit().await.context("commit save workspace tx")?;
        Ok(())
    }

    // ── Environment ops ─────────────────────────────────────────────────────

    pub async fn create_environment(&self, workspace_id: Uuid, name: &str) -> Result<Environment> {
        let env = Environment {
            id: Uuid::new_v4(),
            name: name.to_string(),
            variables: HashMap::new(),
        };
        sqlx::query(
            "INSERT INTO environments (id, workspace_id, name, variables_json)
             VALUES (?, ?, ?, '{}')",
        )
        .bind(env.id.to_string())
        .bind(workspace_id.to_string())
        .bind(&env.name)
        .execute(&self.pool)
        .await
        .context("create environment")?;
        self.touch_workspace(workspace_id).await?;
        Ok(env)
    }

    pub async fn set_active_environment(
        &self,
        workspace_id: Uuid,
        environment_id: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query("UPDATE workspaces SET active_environment_id = ? WHERE id = ?")
            .bind(environment_id.map(|u| u.to_string()))
            .bind(workspace_id.to_string())
            .execute(&self.pool)
            .await
            .context("set active environment")?;
        self.set_session_json(ACTIVE_ENVIRONMENT_ID, &environment_id)
            .await?;
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub(super) async fn insert_workspace_row(&self, ws: &WorkspaceModel) -> Result<()> {
        sqlx::query(
            "INSERT INTO workspaces (id, name, created_at, updated_at, active_environment_id)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(ws.id.to_string())
        .bind(&ws.name)
        .bind(format_rfc3339(ws.created_at))
        .bind(format_rfc3339(ws.updated_at))
        .bind(ws.active_environment_id.map(|u| u.to_string()))
        .execute(&self.pool)
        .await
        .context("insert workspace row")?;
        Ok(())
    }

    pub(super) async fn touch_workspace(&self, workspace_id: Uuid) -> Result<()> {
        let now = format_rfc3339(OffsetDateTime::now_utc());
        sqlx::query("UPDATE workspaces SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(workspace_id.to_string())
            .execute(&self.pool)
            .await
            .context("touch workspace")?;
        Ok(())
    }

    pub(super) async fn load_environments(&self, workspace_id: &str) -> Result<Vec<Environment>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, name, variables_json FROM environments WHERE workspace_id = ? ORDER BY name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load environments")?;

        rows.into_iter()
            .map(|(id, name, vars_json)| {
                let variables: HashMap<String, String> =
                    serde_json::from_str(&vars_json).context("parse env vars")?;
                Ok(Environment {
                    id: parse_uuid(&id)?,
                    name,
                    variables,
                })
            })
            .collect()
    }
}
