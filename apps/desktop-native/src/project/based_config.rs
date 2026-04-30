//! Parses the real `.based/config.toml` shape (`[connection.id]`, `engine`, …).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::connection::{ConnectionConfig, ConnectionEntry};
use crate::mongodb::MongoConfig;
use crate::postgres::{PostgresConfig, SslMode};
use crate::sqlite::SqliteConfig;

#[derive(Debug, Deserialize)]
pub struct BasedConfigFile {
    pub version: Option<u64>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub connection: HashMap<String, RawConnection>,
    #[serde(default)]
    pub settings: Option<BasedSettings>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasedSettings {
    #[serde(default)]
    pub query_timeout: Option<u64>,
    #[serde(default)]
    pub max_result_rows: Option<u64>,
    #[serde(default)]
    pub enable_query_cache: Option<bool>,
    #[serde(default)]
    pub cache_ttl: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawConnection {
    pub label: String,
    pub engine: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub ssl: Option<bool>,
    #[serde(default)]
    pub url: Option<MongoUrlSpec>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum MongoUrlSpec {
    Literal(String),
    FromEnv { env: String },
}

pub fn load_based_config(project_root: &Path) -> Result<BasedConfigFile> {
    let path = project_root.join(".based").join("config.toml");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&raw).context("parse config.toml")
}

fn fallback_project_title(project_root: &Path) -> String {
    project_root
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Project")
        .to_string()
}

/// Project name for the top bar plus connection entries from `[connection.*]` tables.
pub fn load_workspace_seed(project_root: &Path) -> (String, Vec<ConnectionEntry>) {
    let file = match load_based_config(project_root) {
        Ok(f) => f,
        Err(e) => {
            log::warn!(
                "could not load .based/config.toml under {}: {e:#}",
                project_root.display()
            );
            return (fallback_project_title(project_root), vec![]);
        }
    };

    let title = file
        .name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback_project_title(project_root));

    let mut keys: Vec<_> = file.connection.keys().cloned().collect();
    keys.sort();
    let mut entries = Vec::new();
    for key in keys {
        let Some(raw) = file.connection.get(&key) else {
            continue;
        };
        match raw_into_entry(&key, raw) {
            Ok(e) => entries.push(e),
            Err(e) => log::warn!("connection [{key}] skipped: {e:#}"),
        }
    }

    (title, entries)
}

fn raw_into_entry(stable_key: &str, raw: &RawConnection) -> Result<ConnectionEntry> {
    let engine = raw.engine.to_lowercase();
    let config = match engine.as_str() {
        "sqlite" => {
            let file = raw
                .file
                .as_ref()
                .context("sqlite connection requires `file`")?;
            ConnectionConfig::SQLite(SqliteConfig {
                label: raw.label.clone(),
                path: PathBuf::from(file),
                wal: true,
            })
        }
        "postgres" | "postgresql" => {
            let host = raw
                .host
                .clone()
                .context("postgres connection requires `host`")?;
            let database = raw
                .database
                .clone()
                .context("postgres connection requires `database`")?;
            let username = raw
                .username
                .clone()
                .context("postgres connection requires `username`")?;
            let ssl_mode = match raw.ssl {
                Some(false) => SslMode::Disable,
                Some(true) => SslMode::Require,
                None => SslMode::Prefer,
            };
            ConnectionConfig::Postgres(PostgresConfig {
                label: raw.label.clone(),
                host,
                port: raw.port.unwrap_or(5432),
                database,
                username,
                password: raw.password.clone().unwrap_or_default(),
                ssl_mode,
            })
        }
        "mongodb" | "mongo" => {
            let uri = match &raw.url {
                Some(MongoUrlSpec::Literal(s)) => s.clone(),
                Some(MongoUrlSpec::FromEnv { env }) => std::env::var(env).unwrap_or_default(),
                None => anyhow::bail!(
                    "mongodb connection requires `url` (string or {{ env = \"VAR\" }})"
                ),
            };
            if uri.trim().is_empty() {
                anyhow::bail!("mongodb URI is empty (is the env var set?)");
            }
            ConnectionConfig::MongoDB(MongoConfig {
                label: raw.label.clone(),
                uri,
                database: raw.database.clone(),
                auth_source: None,
            })
        }
        other => anyhow::bail!("unknown engine {other:?}"),
    };

    Ok(ConnectionEntry::with_stable_id(config, stable_key))
}
