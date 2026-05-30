use based_project::{ConnectionSpec, PragmaSettings, ProjectConnection};
use based_sqlite::SqlitePragma;

use crate::connection::{ConnectionConfig, ConnectionEntry};
use crate::mongodb::MongoConfig;
use crate::postgres::{PostgresConfig, SslMode};

pub fn entry_from_project(conn: &ProjectConnection) -> anyhow::Result<ConnectionEntry> {
    let config = match &conn.spec {
        ConnectionSpec::Sqlite { file, pragma } => {
            ConnectionConfig::SQLite(crate::sqlite::SqliteConfig {
                label: conn.label.clone(),
                path: file.clone(),
                wal: pragma
                    .as_ref()
                    .and_then(|p| p.journal_mode.as_deref())
                    .is_none_or(|m| m.eq_ignore_ascii_case("wal")),
                pragma: pragma.as_ref().map(map_pragma),
            })
        }
        ConnectionSpec::Postgres {
            host,
            port,
            database,
            username,
            password,
            ssl,
        } => ConnectionConfig::Postgres(PostgresConfig {
            label: conn.label.clone(),
            host: host.clone(),
            port: *port,
            database: database.clone(),
            username: username.clone(),
            password: password.resolve(),
            ssl_mode: if *ssl {
                SslMode::Require
            } else {
                SslMode::Disable
            },
        }),
        ConnectionSpec::MongoDB { url, database } => {
            let uri = url.resolve();
            if uri.trim().is_empty() {
                anyhow::bail!("mongodb connection {} has empty url", conn.id);
            }
            ConnectionConfig::MongoDB(MongoConfig {
                label: conn.label.clone(),
                uri,
                database: database.clone(),
                auth_source: None,
            })
        }
    };
    Ok(ConnectionEntry::with_stable_id_and_tags(
        config,
        &conn.id,
        conn.tags.clone(),
    ))
}

fn map_pragma(p: &PragmaSettings) -> SqlitePragma {
    SqlitePragma {
        journal_mode: p.journal_mode.clone().unwrap_or_else(|| "wal".into()),
        synchronous: p.synchronous.clone().unwrap_or_else(|| "normal".into()),
        foreign_keys: p.foreign_keys.unwrap_or(true),
    }
}
