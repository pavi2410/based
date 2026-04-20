//! Per-connection schema cache.
//!
//! `describe_table` / `describe_collection` are cheap per call but
//! they're invoked every time the user clicks a table, switches the
//! Data/Structure toggle, or opens the row editor. A warm cache cuts
//! that to ~0ms and is a prerequisite for the upcoming SQL
//! autocomplete work (which needs cheap access to every table's
//! columns for symbol resolution).
//!
//! Design goals:
//!  - **Scoped to a `ConnectionId`** so invalidating a disconnected
//!    connection is a single map removal.
//!  - **Manually invalidated** by mutation commands. A generic TTL is
//!    tempting but would either be too short (kills hit rate) or too
//!    long (shows stale schema after `ALTER TABLE`). We invalidate at
//!    the boundaries we actually know about: row mutations invalidate
//!    the describe entry for that table; schema-changing raw SQL
//!    invalidates the whole connection.
//!  - **Lock-free on the read path** once populated: we hold the
//!    outer `RwLock` in read mode and clone the `Arc<TableDescription>`
//!    out. Populating is behind a write guard, which is fine for this
//!    access pattern (one refetch per unique table).
//!
//! The type is deliberately isolated from `ConnectionRegistry` so the
//! two concerns stay independent: a future eviction policy or
//! persistence strategy changes only this file.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::engine::types::TableDescription;

/// Identifier for a cached table/collection within a connection.
/// `schema` is `None` for engines that don't have a namespace concept
/// (SQLite, MongoDB).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CachedObjectKey {
    pub schema: Option<String>,
    pub name: String,
}

impl CachedObjectKey {
    pub fn new(schema: Option<&str>, name: &str) -> Self {
        Self {
            schema: schema.map(str::to_string),
            name: name.to_string(),
        }
    }
}

/// Per-connection descriptions, keyed by `(schema, name)`.
type ConnSchemaMap = HashMap<CachedObjectKey, Arc<TableDescription>>;

/// Schema cache keyed by connection id.
#[derive(Default)]
pub struct SchemaCache {
    inner: RwLock<HashMap<String, ConnSchemaMap>>,
}

impl SchemaCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a cached description. Cheap — only takes the outer read lock.
    pub async fn get(&self, conn_id: &str, key: &CachedObjectKey) -> Option<Arc<TableDescription>> {
        let outer = self.inner.read().await;
        outer.get(conn_id).and_then(|m| m.get(key).cloned())
    }

    /// Populate (or overwrite) a cache entry. Call on the happy path
    /// of a describe command to warm subsequent reads.
    pub async fn put(&self, conn_id: &str, key: CachedObjectKey, value: TableDescription) {
        let mut outer = self.inner.write().await;
        outer
            .entry(conn_id.to_string())
            .or_default()
            .insert(key, Arc::new(value));
    }

    /// Drop the cached description for a single table/collection.
    /// Called after a row mutation so the next Structure view re-reads
    /// (estimated_row_count might have changed).
    pub async fn invalidate(&self, conn_id: &str, key: &CachedObjectKey) {
        let mut outer = self.inner.write().await;
        if let Some(m) = outer.get_mut(conn_id) {
            m.remove(key);
        }
    }

    /// Nuke every cached description for a connection. Called on
    /// reconnect and on raw SQL execution (which may have been DDL).
    pub async fn invalidate_connection(&self, conn_id: &str) {
        let mut outer = self.inner.write().await;
        outer.remove(conn_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::TableDescription;

    fn sample_desc(name: &str) -> TableDescription {
        TableDescription {
            name: name.to_string(),
            schema: None,
            kind: "table".to_string(),
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            row_count: None,
        }
    }

    #[tokio::test]
    async fn get_returns_none_when_empty() {
        let cache = SchemaCache::new();
        assert!(
            cache
                .get("c1", &CachedObjectKey::new(None, "users"))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn put_then_get_roundtrips() {
        let cache = SchemaCache::new();
        let key = CachedObjectKey::new(None, "users");
        cache.put("c1", key.clone(), sample_desc("users")).await;
        let got = cache.get("c1", &key).await;
        assert!(got.is_some());
        assert_eq!(got.unwrap().name, "users");
    }

    #[tokio::test]
    async fn invalidate_drops_entry() {
        let cache = SchemaCache::new();
        let key = CachedObjectKey::new(None, "users");
        cache.put("c1", key.clone(), sample_desc("users")).await;
        cache.invalidate("c1", &key).await;
        assert!(cache.get("c1", &key).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_connection_clears_all() {
        let cache = SchemaCache::new();
        cache
            .put("c1", CachedObjectKey::new(None, "a"), sample_desc("a"))
            .await;
        cache
            .put("c1", CachedObjectKey::new(None, "b"), sample_desc("b"))
            .await;
        cache.invalidate_connection("c1").await;
        assert!(
            cache
                .get("c1", &CachedObjectKey::new(None, "a"))
                .await
                .is_none()
        );
        assert!(
            cache
                .get("c1", &CachedObjectKey::new(None, "b"))
                .await
                .is_none()
        );
    }
}
