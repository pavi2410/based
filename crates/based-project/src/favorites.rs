use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const FAVORITES_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FavoritesFile {
    pub schema_version: u64,
    #[serde(default, rename = "favorite")]
    pub favorites: Vec<FavoriteEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FavoriteEntry {
    pub path: String,
}

pub fn load_favorites(project_root: &Path) -> Result<Vec<String>> {
    let path = project_root
        .join(".based")
        .join("state")
        .join("favorites.toml");
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let file: FavoritesFile =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if file.schema_version != FAVORITES_SCHEMA_VERSION {
        bail!(
            "unsupported favorites schema_version {} (expected {FAVORITES_SCHEMA_VERSION})",
            file.schema_version
        );
    }
    Ok(file.favorites.into_iter().map(|f| f.path).collect())
}

pub fn persist_favorites(project_root: &Path, paths: &[String]) -> Result<()> {
    let dir = project_root.join(".based").join("state");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("favorites.toml");
    let file = FavoritesFile {
        schema_version: FAVORITES_SCHEMA_VERSION,
        favorites: paths
            .iter()
            .map(|p| FavoriteEntry { path: p.clone() })
            .collect(),
    };
    let content = toml::to_string_pretty(&file)?;
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
