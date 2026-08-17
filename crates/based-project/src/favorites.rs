use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::walk::strip_based_toml_suffix;

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
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let file: FavoritesFile =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if file.schema_version != FAVORITES_SCHEMA_VERSION {
        bail!(
            "unsupported favorites schema_version {} (expected {FAVORITES_SCHEMA_VERSION})",
            file.schema_version
        );
    }
    Ok(file
        .favorites
        .into_iter()
        .map(|f| normalize_query_path(&f.path))
        .collect())
}

fn normalize_query_path(path: &str) -> String {
    strip_based_toml_suffix(&path.replace('\\', "/")).to_string()
}

pub fn persist_favorites(project_root: &Path, paths: &[String]) -> Result<()> {
    let dir = project_root.join(".based").join("state");
    fs::create_dir_all(&dir)?;
    let path = dir.join("favorites.toml");
    let file = FavoritesFile {
        schema_version: FAVORITES_SCHEMA_VERSION,
        favorites: paths
            .iter()
            .map(|p| FavoriteEntry {
                path: normalize_query_path(p),
            })
            .collect(),
    };
    let content = toml::to_string_pretty(&file)?;
    fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_query_path_strips_toml_and_legacy_midfix() {
        assert_eq!(
            normalize_query_path("northwind/recent-orders.query.toml"),
            "northwind/recent-orders"
        );
        assert_eq!(
            normalize_query_path("northwind/recent-orders.toml"),
            "northwind/recent-orders"
        );
        assert_eq!(
            normalize_query_path("northwind/recent-orders"),
            "northwind/recent-orders"
        );
    }
}
