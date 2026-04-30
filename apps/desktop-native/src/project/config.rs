// .based/config.toml read/write.
// Implemented in Phase 2.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::connection::ConnectionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub connection: std::collections::HashMap<String, ConnectionConfig>,
}

pub fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".based").join("config.toml")
}

// TODO Phase 2: implement read_config / write_config
