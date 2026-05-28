use std::path::Path;

use based_core::ConnectionId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    pub connection: ConnectionId,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
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
    queries: Vec<SavedQuery>,
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
