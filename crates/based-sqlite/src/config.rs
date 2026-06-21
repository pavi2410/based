use std::env;
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
    pub read_only: bool,
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
    if let Ok(dir) = env::var("BASED_PROJECT_DIR") {
        return PathBuf::from(dir).join(path);
    }
    if let Some(root) = &ctx.project_dir {
        return root.join(path);
    }
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

/// Connection open parameters for [`sqlite_connect_options`].
#[derive(Debug, Clone, Copy)]
pub struct SqliteOpenOptions<'a> {
    pub path: &'a Path,
    pub read_only: bool,
}

pub fn sqlite_connect_options(opts: &SqliteOpenOptions<'_>) -> SqliteConnectOptions {
    let mut o = SqliteConnectOptions::new().filename(opts.path);
    if opts.read_only {
        o = o.read_only(true);
    } else if !opts.path.exists() {
        o = o.create_if_missing(true);
    }
    o
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

    #[test]
    fn read_only_connect_options_builds() {
        let missing = PathBuf::from("/nonexistent/based-test-missing.db");
        let _opts = sqlite_connect_options(&SqliteOpenOptions {
            path: &missing,
            read_only: true,
        });
    }

    #[tokio::test]
    async fn read_only_rejects_insert() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ro.db");
        {
            let pool = sqlx::SqlitePool::connect_with(sqlite_connect_options(&SqliteOpenOptions {
                path: &path,
                read_only: false,
            }))
            .await
            .unwrap();
            sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let pool = sqlx::SqlitePool::connect_with(sqlite_connect_options(&SqliteOpenOptions {
            path: &path,
            read_only: true,
        }))
        .await
        .unwrap();
        let err = sqlx::query("INSERT INTO t (v) VALUES ('x')")
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("read"),
            "expected read-only error, got: {err}"
        );
    }
}
