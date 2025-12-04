use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tokio::sync::RwLock;

use crate::connection_pool::ConnectionPool;
use crate::project_types::Engine;

/// A stable identifier for a database connection.
/// Generated from (project_path, conn_key) using a hash function.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(String);

impl ConnectionId {
    /// Generate a ConnectionId from project path and connection key.
    /// The ID is a hex-encoded hash of the combined inputs.
    pub fn new(project_path: &str, conn_key: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        project_path.hash(&mut hasher);
        conn_key.hash(&mut hasher);
        let hash = hasher.finish();
        Self(format!("{:016x}", hash))
    }

    /// Get the string representation of the ID.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ConnectionId> for String {
    fn from(id: ConnectionId) -> Self {
        id.0
    }
}

impl From<&str> for ConnectionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ConnectionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Metadata about a connection stored in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: ConnectionId,
    pub project_path: String,
    pub conn_key: String,
    pub engine: Engine,
    pub label: Option<String>,
}

/// Central registry for managing database connections.
/// Maps ConnectionId to both metadata and the actual connection pool.
#[derive(Default)]
pub struct ConnectionRegistry {
    /// Connection metadata indexed by ID
    info: RwLock<HashMap<String, ConnectionInfo>>,
    /// Active connection pools indexed by ID
    pools: RwLock<HashMap<String, ConnectionPool>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new connection with its pool.
    pub async fn register(
        &self,
        project_path: String,
        conn_key: String,
        engine: Engine,
        label: Option<String>,
        pool: ConnectionPool,
    ) -> ConnectionId {
        let id = ConnectionId::new(&project_path, &conn_key);
        let id_str = id.as_str().to_string();

        let info = ConnectionInfo {
            id: id.clone(),
            project_path,
            conn_key,
            engine,
            label,
        };

        {
            let mut info_map = self.info.write().await;
            info_map.insert(id_str.clone(), info);
        }

        {
            let mut pool_map = self.pools.write().await;
            pool_map.insert(id_str, pool);
        }

        id
    }

    /// Check if a connection exists by ID.
    pub async fn contains(&self, id: &ConnectionId) -> bool {
        let pools = self.pools.read().await;
        pools.contains_key(id.as_str())
    }

    /// Check if a connection exists by project path and key.
    pub async fn contains_by_key(&self, project_path: &str, conn_key: &str) -> bool {
        let id = ConnectionId::new(project_path, conn_key);
        self.contains(&id).await
    }

    /// Get connection info by ID string.
    pub async fn get_info_by_str(&self, id: &str) -> Option<ConnectionInfo> {
        let info_map = self.info.read().await;
        info_map.get(id).cloned()
    }

    /// Get read access to the pools map.
    /// Use this for executing queries on connections.
    pub async fn pools(&self) -> tokio::sync::RwLockReadGuard<'_, HashMap<String, ConnectionPool>> {
        self.pools.read().await
    }

    /// Close and remove a connection by ID.
    pub async fn close(&self, id: &ConnectionId) {
        let id_str = id.as_str();

        // Remove and close pool
        let pool = {
            let mut pools = self.pools.write().await;
            pools.remove(id_str)
        };

        if let Some(pool) = pool {
            pool.close().await;
        }

        // Remove info
        {
            let mut info_map = self.info.write().await;
            info_map.remove(id_str);
        }
    }

    /// Close all connections for a project.
    pub async fn close_project(&self, project_path: &str) {
        // Find all connections for this project
        let ids_to_close: Vec<ConnectionId> = {
            let info_map = self.info.read().await;
            info_map
                .values()
                .filter(|info| info.project_path == project_path)
                .map(|info| info.id.clone())
                .collect()
        };

        // Close each connection
        for id in ids_to_close {
            self.close(&id).await;
        }
    }

    /// Get the ID for a project connection (without checking if it exists).
    pub fn get_id(project_path: &str, conn_key: &str) -> ConnectionId {
        ConnectionId::new(project_path, conn_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_id_generation() {
        let id1 = ConnectionId::new("/path/to/project", "dev");
        let id2 = ConnectionId::new("/path/to/project", "dev");
        let id3 = ConnectionId::new("/path/to/project", "prod");
        let id4 = ConnectionId::new("/other/project", "dev");

        // Same inputs produce same ID
        assert_eq!(id1, id2);

        // Different inputs produce different IDs
        assert_ne!(id1, id3);
        assert_ne!(id1, id4);
        assert_ne!(id3, id4);
    }

    #[test]
    fn test_connection_id_display() {
        let id = ConnectionId::new("/path/to/project", "dev");
        let s = id.to_string();
        assert_eq!(s.len(), 16); // 64-bit hash as hex
    }
}
