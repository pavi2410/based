use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SqlitePragma {
    #[serde(default = "default_journal_mode")]
    pub journal_mode: String,
    #[serde(default = "default_synchronous")]
    pub synchronous: String,
    #[serde(default = "default_foreign_keys")]
    pub foreign_keys: bool,
}

fn default_journal_mode() -> String {
    "wal".into()
}

fn default_synchronous() -> String {
    "normal".into()
}

fn default_foreign_keys() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub label: String,
    pub path: PathBuf,
    #[serde(default)]
    pub pragma: Option<SqlitePragma>,
}

/// Hints for resolving relative database file paths without a GUI dependency.
#[derive(Debug, Clone, Default)]
pub struct SqlitePathContext {
    /// Explicit project directory (e.g. from `BASED_PROJECT_DIR` or `.based` ancestor).
    pub project_dir: Option<PathBuf>,
}

/// Resolve relative DB paths against `project_dir`, then the process working directory.
/// Absolute paths are unchanged.
pub fn resolve_sqlite_path(path: &Path, ctx: &SqlitePathContext) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if let Ok(dir) = std::env::var("BASED_PROJECT_DIR") {
        return PathBuf::from(dir).join(path);
    }
    if let Some(root) = &ctx.project_dir {
        return root.join(path);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

pub fn sqlite_connect_options(path: &Path, create_if_missing: bool) -> SqliteConnectOptions {
    let mut opts = SqliteConnectOptions::new().filename(path);
    if create_if_missing {
        opts = opts.create_if_missing(true);
    }
    opts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_unchanged() {
        let p = PathBuf::from("/tmp/test.db");
        let ctx = SqlitePathContext::default();
        assert_eq!(resolve_sqlite_path(&p, &ctx), p);
    }

    #[test]
    fn relative_uses_project_dir() {
        let ctx = SqlitePathContext {
            project_dir: Some(PathBuf::from("/project")),
        };
        assert_eq!(
            resolve_sqlite_path(Path::new("app.db"), &ctx),
            PathBuf::from("/project/app.db")
        );
    }
}
