use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

pub const PROJECT_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: u64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub settings: Option<ProjectSettings>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSettings {
    #[serde(default)]
    pub query_timeout: Option<u64>,
    #[serde(default)]
    pub max_result_rows: Option<u64>,
    #[serde(default)]
    pub enable_query_cache: Option<bool>,
    #[serde(default)]
    pub cache_ttl: Option<u64>,
}

pub fn load_manifest(project_root: &Path) -> Result<ProjectManifest> {
    let path = project_root.join(".based").join("project.toml");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let manifest: ProjectManifest =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if manifest.schema_version != PROJECT_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported project.toml schema_version {} (expected {PROJECT_SCHEMA_VERSION})",
            manifest.schema_version
        );
    }
    Ok(manifest)
}
