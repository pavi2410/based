use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::connection::ConnectionId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    pub connection: ConnectionId,
    pub tags: Vec<String>,
    /// SQL text (Postgres/SQLite) or pipeline JSON string (MongoDB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
    /// MongoDB collection name when `pipeline` is set (optional; defaults for palette runs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mongo_collection: Option<String>,
}

impl SavedQuery {
    pub fn query_text(&self) -> &str {
        self.sql
            .as_deref()
            .or(self.pipeline.as_deref())
            .unwrap_or("")
    }
}

#[derive(Default, Serialize, Deserialize)]
struct SavedFile {
    #[serde(default, rename = "query")]
    pub queries: Vec<SavedQuery>,
}

pub struct SavedQueries {
    pub queries: Vec<SavedQuery>,
}

impl SavedQueries {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self { queries: vec![] };
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let file: SavedFile = toml::from_str(&content).unwrap_or_default();
        Self {
            queries: file.queries,
        }
    }

    pub fn add(&mut self, query: SavedQuery) {
        if let Some(existing) = self.queries.iter_mut().find(|q| q.id == query.id) {
            *existing = query;
        } else {
            self.queries.push(query);
        }
    }

    pub fn persist(&self, path: &Path) {
        let file = SavedFile {
            queries: self.queries.clone(),
        };
        if let Ok(content) = toml::to_string_pretty(&file) {
            let _ = std::fs::write(path, content);
        }
    }

    pub fn for_conn(&self, conn_id: &ConnectionId) -> Vec<&SavedQuery> {
        self.queries
            .iter()
            .filter(|q| &q.connection == conn_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_query(id: &str, name: &str) -> SavedQuery {
        SavedQuery {
            id: id.into(),
            name: name.into(),
            connection: ConnectionId("pg".into()),
            tags: vec!["test".into()],
            sql: Some("SELECT 1".into()),
            pipeline: None,
            mongo_collection: None,
        }
    }

    #[test]
    fn add_and_persist_and_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("queries.toml");
        let mut s = SavedQueries::load(&path);
        s.add(make_query("q_1", "My Query"));
        s.persist(&path);

        let s2 = SavedQueries::load(&path);
        assert_eq!(s2.queries.len(), 1);
        assert_eq!(s2.queries[0].name, "My Query");
    }

    #[test]
    fn add_replaces_same_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("queries.toml");
        let mut s = SavedQueries::load(&path);
        s.add(make_query("q_1", "Original"));
        s.add(make_query("q_1", "Updated"));
        assert_eq!(s.queries.len(), 1);
        assert_eq!(s.queries[0].name, "Updated");
    }
}
