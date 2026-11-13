use std::env;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqlx::sqlite::SqliteConnectOptions;

/// Pragmas applied when a connection opens. Defaults: WAL, NORMAL, foreign keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlitePragma {
    pub journal_mode: JournalMode,
    pub synchronous: Synchronous,
    pub foreign_keys: bool,
}

impl Default for SqlitePragma {
    fn default() -> Self {
        Self {
            journal_mode: JournalMode::default(),
            synchronous: Synchronous::default(),
            foreign_keys: true,
        }
    }
}

impl SqlitePragma {
    pub fn foreign_keys_pragma(&self) -> &'static str {
        if self.foreign_keys {
            "PRAGMA foreign_keys=ON"
        } else {
            "PRAGMA foreign_keys=OFF"
        }
    }
}

/// A pragma value a connection file names that SQLite doesn't have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPragmaValue {
    pub pragma: &'static str,
    pub value: String,
}

impl fmt::Display for UnknownPragmaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported {}: {}", self.pragma, self.value)
    }
}

impl Error for UnknownPragmaValue {}

/// `PRAGMA journal_mode`, written lowercase in connection files.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    #[default]
    Wal,
    Off,
}

impl JournalMode {
    const ALL: [Self; 6] = [
        Self::Delete,
        Self::Truncate,
        Self::Persist,
        Self::Memory,
        Self::Wal,
        Self::Off,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Truncate => "truncate",
            Self::Persist => "persist",
            Self::Memory => "memory",
            Self::Wal => "wal",
            Self::Off => "off",
        }
    }

    pub fn pragma(self) -> &'static str {
        match self {
            Self::Delete => "PRAGMA journal_mode=DELETE",
            Self::Truncate => "PRAGMA journal_mode=TRUNCATE",
            Self::Persist => "PRAGMA journal_mode=PERSIST",
            Self::Memory => "PRAGMA journal_mode=MEMORY",
            Self::Wal => "PRAGMA journal_mode=WAL",
            Self::Off => "PRAGMA journal_mode=OFF",
        }
    }
}

impl FromStr for JournalMode {
    type Err = UnknownPragmaValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|m| m.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownPragmaValue {
                pragma: "journal_mode",
                value: s.to_string(),
            })
    }
}

/// `PRAGMA synchronous`, written lowercase in connection files.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Synchronous {
    Off,
    #[default]
    Normal,
    Full,
    Extra,
}

impl Synchronous {
    const ALL: [Self; 4] = [Self::Off, Self::Normal, Self::Full, Self::Extra];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Normal => "normal",
            Self::Full => "full",
            Self::Extra => "extra",
        }
    }

    pub fn pragma(self) -> &'static str {
        match self {
            Self::Off => "PRAGMA synchronous=OFF",
            Self::Normal => "PRAGMA synchronous=NORMAL",
            Self::Full => "PRAGMA synchronous=FULL",
            Self::Extra => "PRAGMA synchronous=EXTRA",
        }
    }
}

impl FromStr for Synchronous {
    type Err = UnknownPragmaValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|m| m.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownPragmaValue {
                pragma: "synchronous",
                value: s.to_string(),
            })
    }
}

#[derive(Debug, Clone)]
pub struct SqliteConfig {
    pub label: String,
    pub path: PathBuf,
    pub read_only: bool,
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

/// SQLite URI (`file:…`). `read_only` adds `?mode=ro`.
pub fn sqlite_uri(path: &Path, read_only: bool) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let encoded = percent_encode_path(&raw);
    let uri = if encoded.starts_with('/') {
        format!("file:{encoded}")
    } else {
        format!("file:///{encoded}")
    };
    if read_only {
        format!("{uri}?mode=ro")
    } else {
        uri
    }
}

/// `sqlite3` invocation for the given database file.
pub fn sqlite3_command(path: &Path, read_only: bool) -> String {
    let quoted = shell_single_quote(&path.to_string_lossy());
    if read_only {
        format!("sqlite3 -readonly {quoted}")
    } else {
        format!("sqlite3 {quoted}")
    }
}

fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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
    fn pragma_defaults_are_wal_normal_foreign_keys_on() {
        let p = SqlitePragma::default();
        assert_eq!(p.journal_mode, JournalMode::Wal);
        assert_eq!(p.synchronous, Synchronous::Normal);
        assert!(p.foreign_keys);
    }

    #[test]
    fn pragma_values_parse_case_insensitively_and_round_trip() {
        assert_eq!("WAL".parse::<JournalMode>(), Ok(JournalMode::Wal));
        assert_eq!("extra".parse::<Synchronous>(), Ok(Synchronous::Extra));
        for m in JournalMode::ALL {
            assert_eq!(m.as_str().parse::<JournalMode>(), Ok(m));
        }
        for m in Synchronous::ALL {
            assert_eq!(m.as_str().parse::<Synchronous>(), Ok(m));
        }
        let err = "fast".parse::<Synchronous>().unwrap_err();
        assert_eq!(err.to_string(), "unsupported synchronous: fast");
    }

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

    #[test]
    fn uri_uses_file_scheme() {
        assert_eq!(
            sqlite_uri(Path::new("/tmp/app.db"), false),
            "file:/tmp/app.db"
        );
    }

    #[test]
    fn uri_marks_read_only() {
        assert_eq!(
            sqlite_uri(Path::new("/tmp/app.db"), true),
            "file:/tmp/app.db?mode=ro"
        );
    }

    #[test]
    fn uri_percent_encodes_path() {
        assert_eq!(
            sqlite_uri(Path::new("/tmp/my db.db"), false),
            "file:/tmp/my%20db.db"
        );
    }

    #[test]
    fn sqlite3_quotes_path() {
        assert_eq!(
            sqlite3_command(Path::new("/tmp/app.db"), false),
            "sqlite3 '/tmp/app.db'"
        );
    }

    #[test]
    fn sqlite3_readonly_flag() {
        assert_eq!(
            sqlite3_command(Path::new("/tmp/app.db"), true),
            "sqlite3 -readonly '/tmp/app.db'"
        );
    }

    #[test]
    fn sqlite3_escapes_single_quotes() {
        assert_eq!(
            sqlite3_command(Path::new("/tmp/o'reilly.db"), false),
            "sqlite3 '/tmp/o'\\''reilly.db'"
        );
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
