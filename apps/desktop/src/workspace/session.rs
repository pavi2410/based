//! Persist open tab specs to `.based/local/workspace.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::TabSpec;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    pub tabs: Vec<TabSpec>,
    pub active: Option<usize>,
}

impl SessionState {
    pub fn path(project_root: &Path) -> PathBuf {
        project_root
            .join(".based")
            .join("local")
            .join("workspace.json")
    }

    pub fn load(project_root: &Path) -> Self {
        let path = Self::path(project_root);
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        std::fs::write(path, body)
    }
}
