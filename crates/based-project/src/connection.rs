use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::env_value::EnvOrString;
use crate::walk::walk_files;

pub const CONNECTION_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub struct ProjectConnection {
    pub id: String,
    pub label: String,
    pub engine: String,
    pub tags: Vec<String>,
    pub read_only: bool,
    pub spec: ConnectionSpec,
}

#[derive(Debug, Clone)]
pub enum ConnectionSpec {
    Sqlite {
        file: PathBuf,
        pragma: Option<PragmaSettings>,
    },
    Postgres {
        host: String,
        port: u16,
        database: String,
        username: String,
        password: EnvOrString,
        ssl: bool,
    },
    MongoDB {
        url: EnvOrString,
        database: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PragmaSettings {
    #[serde(default)]
    pub journal_mode: Option<String>,
    #[serde(default)]
    pub synchronous: Option<String>,
    #[serde(default)]
    pub foreign_keys: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawConnectionFile {
    schema_version: u64,
    label: String,
    engine: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    database: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<EnvOrString>,
    #[serde(default)]
    ssl: Option<bool>,
    #[serde(default)]
    url: Option<EnvOrString>,
    #[serde(default)]
    pragma: Option<PragmaSettings>,
    #[serde(default)]
    read_only: Option<bool>,
}

pub fn load_connections(project_root: &Path) -> Result<Vec<ProjectConnection>> {
    let dir = project_root.join(".based").join("connections");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let files = walk_files(&dir, ".conn.toml")?;
    let mut connections = Vec::with_capacity(files.len());
    for path in files {
        connections.push(parse_connection_file(&dir, &path)?);
    }
    Ok(connections)
}

fn parse_connection_file(connections_dir: &Path, path: &Path) -> Result<ProjectConnection> {
    let rel = path
        .strip_prefix(connections_dir)
        .with_context(|| format!("connection path not under {}", connections_dir.display()))?;
    let id = connection_id_from_rel_path(rel);
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file: RawConnectionFile =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if file.schema_version != CONNECTION_SCHEMA_VERSION {
        bail!(
            "unsupported schema_version {} in {} (expected {CONNECTION_SCHEMA_VERSION})",
            file.schema_version,
            path.display()
        );
    }
    let engine = file.engine.to_lowercase();
    let spec = match engine.as_str() {
        "sqlite" => {
            let file_path = file
                .file
                .as_ref()
                .with_context(|| format!("sqlite connection {id} requires `file`"))?;
            ConnectionSpec::Sqlite {
                file: PathBuf::from(file_path),
                pragma: file.pragma,
            }
        }
        "postgres" | "postgresql" => ConnectionSpec::Postgres {
            host: file
                .host
                .clone()
                .with_context(|| format!("postgres connection {id} requires `host`"))?,
            port: file.port.unwrap_or(5432),
            database: file
                .database
                .clone()
                .with_context(|| format!("postgres connection {id} requires `database`"))?,
            username: file
                .username
                .clone()
                .with_context(|| format!("postgres connection {id} requires `username`"))?,
            password: file
                .password
                .clone()
                .unwrap_or(EnvOrString::Literal(String::new())),
            ssl: file.ssl.unwrap_or(false),
        },
        "mongodb" | "mongo" => {
            let url = file
                .url
                .clone()
                .with_context(|| format!("mongodb connection {id} requires `url`"))?;
            ConnectionSpec::MongoDB {
                url,
                database: file.database.clone(),
            }
        }
        other => bail!("unknown engine {other:?} in {}", path.display()),
    };
    let read_only = resolve_read_only(file.read_only, &file.tags);
    Ok(ProjectConnection {
        id,
        label: file.label,
        engine,
        tags: file.tags,
        read_only,
        spec,
    })
}

fn resolve_read_only(explicit: Option<bool>, tags: &[String]) -> bool {
    if let Some(v) = explicit {
        return v;
    }
    tags.iter().any(|t| t.eq_ignore_ascii_case("readonly"))
}

fn connection_id_from_rel_path(rel: &Path) -> String {
    let s = rel.to_string_lossy();
    s.strip_suffix(".conn.toml")
        .unwrap_or(&s)
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_explicit_true() {
        assert!(resolve_read_only(Some(true), &[]));
    }

    #[test]
    fn read_only_explicit_false_overrides_readonly_tag() {
        assert!(!resolve_read_only(Some(false), &["readonly".into()]));
    }

    #[test]
    fn read_only_from_readonly_tag() {
        assert!(resolve_read_only(None, &["demo".into(), "readonly".into()]));
        assert!(resolve_read_only(None, &["ReadOnly".into()]));
    }

    #[test]
    fn read_only_defaults_false() {
        assert!(!resolve_read_only(None, &["local".into()]));
    }

    #[test]
    fn parse_sqlite_read_only_from_toml() {
        let dir = tempfile::tempdir().unwrap();
        let conn_dir = dir.path().join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        let path = conn_dir.join("index.conn.toml");
        fs::write(
            &path,
            r#"
schema_version = 1
label = "Index"
engine = "sqlite"
read_only = true
file = "data/index.db"
"#,
        )
        .unwrap();
        let conn = parse_connection_file(&conn_dir, &path).unwrap();
        assert!(conn.read_only);
        assert!(matches!(conn.spec, ConnectionSpec::Sqlite { .. }));
    }
}
