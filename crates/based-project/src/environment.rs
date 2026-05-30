use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const ENVIRONMENT_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActiveEnvironment {
    pub schema_version: u64,
    pub name: String,
}

impl Default for ActiveEnvironment {
    fn default() -> Self {
        Self {
            schema_version: ENVIRONMENT_SCHEMA_VERSION,
            name: "default".to_string(),
        }
    }
}

pub fn load_active_environment(project_root: &Path) -> Result<ActiveEnvironment> {
    let path = project_root
        .join(".based")
        .join("state")
        .join("active_environment.toml");
    if !path.exists() {
        return Ok(ActiveEnvironment::default());
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let env: ActiveEnvironment =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if env.schema_version != ENVIRONMENT_SCHEMA_VERSION {
        bail!(
            "unsupported active_environment schema_version {} (expected {ENVIRONMENT_SCHEMA_VERSION})",
            env.schema_version
        );
    }
    Ok(env)
}

pub fn persist_active_environment(project_root: &Path, name: &str) -> Result<()> {
    let dir = project_root.join(".based").join("state");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("active_environment.toml");
    let env = ActiveEnvironment {
        schema_version: ENVIRONMENT_SCHEMA_VERSION,
        name: name.to_string(),
    };
    let content = toml::to_string_pretty(&env)?;
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
