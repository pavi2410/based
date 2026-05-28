//! Workspace session persistence (connection recency, last errors).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceState {
    pub connections: Vec<PersistedConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedConnection {
    pub id: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_connected_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

impl WorkspaceState {
    pub fn load(project_dir: &Path) -> anyhow::Result<Self> {
        let path = Self::path(project_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn save(&self, project_dir: &Path) -> anyhow::Result<()> {
        let path = Self::path(project_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, s)?;
        Ok(())
    }

    pub fn path(project_dir: &Path) -> PathBuf {
        project_dir
            .join(".based")
            .join("state")
            .join("workspace.json")
    }
}
