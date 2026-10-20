//! SQLite WAL metadata store and workspace persistence.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::migrate;
use crate::paths::{self, default_db_path};
use crate::secrets::SecretStore;

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
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));

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
}

pub(super) fn parse_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).with_context(|| format!("invalid uuid: {s}"))
}

pub(super) fn format_rfc3339(ts: OffsetDateTime) -> String {
    ts.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

pub(super) fn parse_rfc3339(s: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).with_context(|| format!("invalid rfc3339 timestamp: {s}"))
}

mod queries;
mod templates;
mod workspace;

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
