//! SQLite WAL metadata store and workspace persistence.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use based_workspace::{
    Collection, ConnectionTemplate, Environment, LooseQuery, SavedQueryRef, WorkspaceModel,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::migrate;
use crate::paths::{self, default_db_path};
use crate::secrets::SecretStore;
use crate::session_keys::{ACTIVE_ENVIRONMENT_ID, ACTIVE_WORKSPACE_ID};

#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    pub id: Uuid,
    pub name: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MetadataStore {
    pool: SqlitePool,
    secrets: SecretStore,
    path: PathBuf,
}

impl MetadataStore {
    pub async fn open_default() -> Result<Self> {
        Self::open(default_db_path()).await
    }

    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        paths::ensure_parent(&path).context("create metadata db parent dir")?;

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| format!("open metadata db at {}", path.display()))?;

        migrate::run(&pool).await?;

        Ok(Self {
            pool,
            secrets: SecretStore::new(),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    pub async fn checkpoint_wal(&self) -> Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .context("wal checkpoint")?;
        Ok(())
    }

    // ── Session helpers ─────────────────────────────────────────────────────

    pub async fn get_session_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM session_state WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .context("read session_state")?;
        raw.map(|s| serde_json::from_str(&s).context("parse session json"))
            .transpose()
    }

    pub async fn set_session_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).context("serialize session json")?;
        sqlx::query(
            "INSERT INTO session_state (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        )
        .bind(key)
        .bind(json)
        .execute(&self.pool)
        .await
        .context("write session_state")?;
        Ok(())
    }

    pub async fn delete_session_key(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM session_state WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .context("delete session_state key")?;
        Ok(())
    }

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
        if let Some(id) = self.active_workspace_id().await? {
            if let Some(ws) = self.get_workspace(id).await? {
                return Ok(ws);
            }
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

    // ── Connection templates ────────────────────────────────────────────────

    pub async fn upsert_connection_template(
        &self,
        workspace_id: Uuid,
        template: &ConnectionTemplate,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.context("begin upsert template")?;
        let ws_id = workspace_id.to_string();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM connection_templates WHERE workspace_id = ?")
                .bind(&ws_id)
                .fetch_one(&mut *tx)
                .await
                .context("count templates")?;
        self.insert_template(&mut tx, &ws_id, template, count)
            .await?;
        tx.commit().await.context("commit upsert template")?;
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub async fn set_template_password_secret(
        &self,
        template_id: Uuid,
        password: &str,
    ) -> Result<()> {
        let ref_key = SecretStore::template_password_ref(template_id);
        self.secrets.set(&ref_key, password)?;
        sqlx::query(
            "UPDATE connection_templates SET password_secret_ref = ?, password_template = ''
             WHERE id = ?",
        )
        .bind(&ref_key)
        .bind(template_id.to_string())
        .execute(&self.pool)
        .await
        .context("record template secret ref")?;
        Ok(())
    }

    // ── Collections & queries ───────────────────────────────────────────────

    pub async fn create_collection(&self, workspace_id: Uuid, name: &str) -> Result<Collection> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM collections WHERE workspace_id = ?")
                .bind(workspace_id.to_string())
                .fetch_one(&self.pool)
                .await
                .context("count collections")?;
        let collection = Collection {
            id: Uuid::new_v4(),
            name: name.to_string(),
            queries: Vec::new(),
        };
        sqlx::query(
            "INSERT INTO collections (id, workspace_id, name, sort_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(collection.id.to_string())
        .bind(workspace_id.to_string())
        .bind(&collection.name)
        .bind(count)
        .execute(&self.pool)
        .await
        .context("insert collection")?;
        self.touch_workspace(workspace_id).await?;
        Ok(collection)
    }

    pub async fn create_loose_query(
        &self,
        workspace_id: Uuid,
        name: &str,
        sql: &str,
        connection_template_id: Option<Uuid>,
    ) -> Result<LooseQuery> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM queries WHERE workspace_id = ? AND collection_id IS NULL",
        )
        .bind(workspace_id.to_string())
        .fetch_one(&self.pool)
        .await
        .context("count loose queries")?;
        let query = LooseQuery {
            id: Uuid::new_v4(),
            name: name.to_string(),
            sql: sql.to_string(),
            connection_template_id,
        };
        sqlx::query(
            "INSERT INTO queries
             (id, workspace_id, collection_id, name, sql_text, connection_template_id, sort_order)
             VALUES (?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(query.id.to_string())
        .bind(workspace_id.to_string())
        .bind(&query.name)
        .bind(&query.sql)
        .bind(connection_template_id.map(|u| u.to_string()))
        .bind(count)
        .execute(&self.pool)
        .await
        .context("insert loose query")?;
        self.touch_workspace(workspace_id).await?;
        Ok(query)
    }

    pub async fn move_query_to_collection(
        &self,
        workspace_id: Uuid,
        query_id: Uuid,
        collection_id: Uuid,
    ) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM queries WHERE collection_id = ?")
            .bind(collection_id.to_string())
            .fetch_one(&self.pool)
            .await
            .context("count collection queries")?;
        let rows = sqlx::query(
            "UPDATE queries SET collection_id = ?, sort_order = ?
             WHERE id = ? AND workspace_id = ? AND collection_id IS NULL",
        )
        .bind(collection_id.to_string())
        .bind(count)
        .bind(query_id.to_string())
        .bind(workspace_id.to_string())
        .execute(&self.pool)
        .await
        .context("move query to collection")?
        .rows_affected();
        if rows == 0 {
            bail!("loose query not found");
        }
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub async fn move_query_to_loose(&self, workspace_id: Uuid, query_id: Uuid) -> Result<()> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM queries WHERE workspace_id = ? AND collection_id IS NULL",
        )
        .bind(workspace_id.to_string())
        .fetch_one(&self.pool)
        .await
        .context("count loose queries")?;
        let rows = sqlx::query(
            "UPDATE queries SET collection_id = NULL, sort_order = ?
             WHERE id = ? AND workspace_id = ? AND collection_id IS NOT NULL",
        )
        .bind(count)
        .bind(query_id.to_string())
        .bind(workspace_id.to_string())
        .execute(&self.pool)
        .await
        .context("move query to loose")?
        .rows_affected();
        if rows == 0 {
            bail!("collection query not found");
        }
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    // ── Internal loaders ────────────────────────────────────────────────────

    async fn insert_workspace_row(&self, ws: &WorkspaceModel) -> Result<()> {
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

    async fn touch_workspace(&self, workspace_id: Uuid) -> Result<()> {
        let now = format_rfc3339(OffsetDateTime::now_utc());
        sqlx::query("UPDATE workspaces SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(workspace_id.to_string())
            .execute(&self.pool)
            .await
            .context("touch workspace")?;
        Ok(())
    }

    async fn load_environments(&self, workspace_id: &str) -> Result<Vec<Environment>> {
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

    async fn load_templates(&self, workspace_id: &str) -> Result<Vec<ConnectionTemplate>> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        )> = sqlx::query_as(
            "SELECT id, label, host, port, database_name, username, password_template,
                    password_secret_ref, ssl_mode
             FROM connection_templates
             WHERE workspace_id = ?
             ORDER BY sort_order, label",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load templates")?;

        let mut out = Vec::new();
        for (
            id,
            label,
            host,
            port,
            database,
            username,
            password_template,
            password_secret_ref,
            ssl_mode,
        ) in rows
        {
            let id = parse_uuid(&id)?;
            let password = if let Some(ref_key) = password_secret_ref {
                self.secrets.get(&ref_key)?.unwrap_or_default()
            } else {
                password_template
            };
            out.push(ConnectionTemplate {
                id,
                label,
                host,
                port,
                database,
                username,
                password,
                ssl_mode,
            });
        }
        Ok(out)
    }

    async fn load_collections(&self, workspace_id: &str) -> Result<Vec<Collection>> {
        let coll_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, name FROM collections WHERE workspace_id = ? ORDER BY sort_order, name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load collections")?;

        let mut out = Vec::new();
        for (coll_id, name) in coll_rows {
            let queries = self.load_collection_queries(&coll_id).await?;
            out.push(Collection {
                id: parse_uuid(&coll_id)?,
                name,
                queries,
            });
        }
        Ok(out)
    }

    async fn load_collection_queries(&self, collection_id: &str) -> Result<Vec<SavedQueryRef>> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, sql_text, connection_template_id
             FROM queries WHERE collection_id = ? ORDER BY sort_order, name",
        )
        .bind(collection_id)
        .fetch_all(&self.pool)
        .await
        .context("load collection queries")?;

        rows.into_iter()
            .map(|(id, name, sql, template_id)| {
                Ok(SavedQueryRef {
                    id: parse_uuid(&id)?,
                    name,
                    sql,
                    connection_template_id: template_id.map(|s| parse_uuid(&s)).transpose()?,
                })
            })
            .collect()
    }

    async fn load_loose_queries(&self, workspace_id: &str) -> Result<Vec<LooseQuery>> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, sql_text, connection_template_id
             FROM queries
             WHERE workspace_id = ? AND collection_id IS NULL
             ORDER BY sort_order, name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load loose queries")?;

        rows.into_iter()
            .map(|(id, name, sql, template_id)| {
                Ok(LooseQuery {
                    id: parse_uuid(&id)?,
                    name,
                    sql,
                    connection_template_id: template_id.map(|s| parse_uuid(&s)).transpose()?,
                })
            })
            .collect()
    }

    async fn insert_template(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workspace_id: &str,
        template: &ConnectionTemplate,
        sort_order: i64,
    ) -> Result<()> {
        let (password_template, password_secret_ref) =
            split_template_password(template.id, &template.password, &self.secrets)?;

        sqlx::query(
            "INSERT INTO connection_templates
             (id, workspace_id, label, host, port, database_name, username,
              password_template, password_secret_ref, ssl_mode, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               label = excluded.label,
               host = excluded.host,
               port = excluded.port,
               database_name = excluded.database_name,
               username = excluded.username,
               password_template = excluded.password_template,
               password_secret_ref = excluded.password_secret_ref,
               ssl_mode = excluded.ssl_mode,
               sort_order = excluded.sort_order",
        )
        .bind(template.id.to_string())
        .bind(workspace_id)
        .bind(&template.label)
        .bind(&template.host)
        .bind(&template.port)
        .bind(&template.database)
        .bind(&template.username)
        .bind(password_template)
        .bind(password_secret_ref)
        .bind(&template.ssl_mode)
        .bind(sort_order)
        .execute(&mut **tx)
        .await
        .context("insert connection template")?;
        Ok(())
    }
}

fn split_template_password(
    template_id: Uuid,
    password: &str,
    secrets: &SecretStore,
) -> Result<(String, Option<String>)> {
    if password.is_empty() {
        return Ok((String::new(), None));
    }
    if password.contains("{{") {
        return Ok((password.to_string(), None));
    }
    let ref_key = SecretStore::template_password_ref(template_id);
    secrets.set(&ref_key, password)?;
    Ok((String::new(), Some(ref_key)))
}

fn parse_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).with_context(|| format!("invalid uuid: {s}"))
}

fn format_rfc3339(ts: OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn parse_rfc3339(s: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .with_context(|| format!("invalid rfc3339 timestamp: {s}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_default_workspace_and_persists_queries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.db");
        let store = MetadataStore::open(&path).await.unwrap();
        let ws = store.create_workspace("dev").await.unwrap();
        assert_eq!(ws.name, "dev");

        let q = store
            .create_loose_query(ws.id, "smoke", "SELECT 1", None)
            .await
            .unwrap();
        let coll = store.create_collection(ws.id, "daily").await.unwrap();
        store
            .move_query_to_collection(ws.id, q.id, coll.id)
            .await
            .unwrap();

        let loaded = store.get_workspace(ws.id).await.unwrap().unwrap();
        assert!(loaded.loose_queries.is_empty());
        assert_eq!(loaded.collections.len(), 1);
        assert_eq!(loaded.collections[0].queries.len(), 1);
        assert_eq!(loaded.collections[0].queries[0].name, "smoke");
    }

    #[tokio::test]
    async fn move_query_back_to_loose_lane() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("meta.db"))
            .await
            .unwrap();
        let ws = store.create_workspace("dev").await.unwrap();
        let q = store
            .create_loose_query(ws.id, "q1", "SELECT 2", None)
            .await
            .unwrap();
        let coll = store.create_collection(ws.id, "c1").await.unwrap();
        store
            .move_query_to_collection(ws.id, q.id, coll.id)
            .await
            .unwrap();
        store.move_query_to_loose(ws.id, q.id).await.unwrap();

        let loaded = store.get_workspace(ws.id).await.unwrap().unwrap();
        assert_eq!(loaded.loose_queries.len(), 1);
        assert!(loaded.collections[0].queries.is_empty());
    }

    #[tokio::test]
    async fn session_state_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("meta.db"))
            .await
            .unwrap();
        let id = Uuid::new_v4();
        store.set_active_workspace_id(Some(id)).await.unwrap();
        assert_eq!(store.active_workspace_id().await.unwrap(), Some(id));
    }
}
