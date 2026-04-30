// Saved queries — .based/queries/<connection_label>/*.sql
// Each file: name from filename stem, content from file body.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub name: String,
    pub content: String,
    pub connection_label: String,
}

pub fn queries_dir(project_dir: &Path, connection_label: &str) -> PathBuf {
    project_dir
        .join(".based")
        .join("queries")
        .join(connection_label)
}

/// Read all `.sql` files from `.based/queries/<connection_label>/`.
/// Returns an empty vec if the directory does not exist.
pub fn load_queries(project_dir: &Path, connection_label: &str) -> Result<Vec<SavedQuery>> {
    let dir = queries_dir(project_dir, connection_label);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut queries = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let content = std::fs::read_to_string(&path)?;
            queries.push(SavedQuery {
                name,
                content,
                connection_label: connection_label.to_string(),
            });
        }
    }
    Ok(queries)
}

/// Save a query to `.based/queries/<connection_label>/<name>.sql`.
pub fn save_query(project_dir: &Path, query: &SavedQuery) -> Result<()> {
    let dir = queries_dir(project_dir, &query.connection_label);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.sql", query.name));
    std::fs::write(&path, &query.content)?;
    Ok(())
}

/// Delete the `.sql` file for the given query.
pub fn delete_query(project_dir: &Path, query: &SavedQuery) -> Result<()> {
    let path =
        queries_dir(project_dir, &query.connection_label).join(format!("{}.sql", query.name));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
