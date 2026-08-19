use std::collections::HashMap;
use std::path::Path;

use based_core::SshTunnelConfig;
use based_project::{
    ConnectionSpec, PragmaSettings, ProjectConnection, SshSettings,
    load_connections_from_based_dir, load_env_file,
};
use based_sqlite::SqlitePragma;

use crate::connection::{ConnectionConfig, ConnectionEntry, ConnectionId, ConnectionOrigin};
use crate::mongodb::MongoConfig;
use crate::postgres::{PostgresConfig, SslMode};
use crate::sqlite::SqliteConfig;

pub fn entry_from_project(conn: &ProjectConnection) -> anyhow::Result<ConnectionEntry> {
    entry_from_tree(conn, ConnectionOrigin::Project, &HashMap::new())
}

pub fn entry_from_tree(
    conn: &ProjectConnection,
    origin: ConnectionOrigin,
    file_vars: &HashMap<String, String>,
) -> anyhow::Result<ConnectionEntry> {
    let config = match &conn.spec {
        ConnectionSpec::Sqlite { file, pragma } => ConnectionConfig::SQLite(SqliteConfig {
            label: conn.label.clone(),
            path: file.clone(),
            read_only: conn.read_only,
            pragma: pragma.as_ref().map(map_pragma),
        }),
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
            password: password.resolve_with(file_vars)?,
            ssl_mode: if *ssl {
                SslMode::Require
            } else {
                SslMode::Disable
            },
            ssh: conn
                .ssh
                .as_ref()
                .map(|s| resolve_ssh(s, file_vars))
                .transpose()?,
        }),
        ConnectionSpec::MongoDB { url, database } => {
            let uri = url.resolve_with(file_vars)?;
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
    let stable_key = match origin {
        ConnectionOrigin::Project => conn.id.clone(),
        ConnectionOrigin::Personal => ConnectionId::personal(&conn.id).0,
    };
    Ok(ConnectionEntry::with_origin(
        config,
        &stable_key,
        conn.tags.clone(),
        origin,
    ))
}

pub fn load_entries_from_based_dir(
    based_dir: &Path,
    origin: ConnectionOrigin,
) -> Vec<ConnectionEntry> {
    let file_vars = load_env_file(&based_dir.join(".env")).unwrap_or_default();
    let connections = match load_connections_from_based_dir(based_dir) {
        Ok(c) => c,
        Err(err) => {
            log::warn!(
                "load connections from {} failed: {err:#}",
                based_dir.display()
            );
            return Vec::new();
        }
    };
    connections
        .iter()
        .filter_map(|conn| match entry_from_tree(conn, origin, &file_vars) {
            Ok(entry) => Some(entry),
            Err(err) => {
                log::warn!("connection {} skipped: {err:#}", conn.id);
                None
            }
        })
        .collect()
}

fn resolve_ssh(
    ssh: &SshSettings,
    file_vars: &HashMap<String, String>,
) -> anyhow::Result<SshTunnelConfig> {
    Ok(SshTunnelConfig {
        host: ssh.host.clone(),
        port: ssh.port,
        user: ssh.user.clone(),
        key_path: ssh.key_path.clone(),
        key_passphrase: ssh
            .key_passphrase
            .as_ref()
            .map(|v| v.resolve_with(file_vars))
            .transpose()?,
    })
}

fn map_pragma(p: &PragmaSettings) -> SqlitePragma {
    SqlitePragma {
        journal_mode: p.journal_mode.clone().unwrap_or_else(|| "wal".into()),
        synchronous: p.synchronous.clone().unwrap_or_else(|| "normal".into()),
        foreign_keys: p.foreign_keys.unwrap_or(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use based_project::{EnvOrString, write_connection_file};
    use std::path::PathBuf;

    #[test]
    fn personal_tree_entry_uses_user_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let conn = ProjectConnection {
            id: "analytics".into(),
            label: "Analytics".into(),
            engine: "sqlite".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Sqlite {
                file: PathBuf::from("/tmp/a.db"),
                pragma: None,
            },
            ssh: None,
        };
        write_connection_file(based, &conn).unwrap();
        let entries = load_entries_from_based_dir(based, ConnectionOrigin::Personal);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, ConnectionId::personal("analytics"));
        assert_eq!(entries[0].origin, ConnectionOrigin::Personal);
    }

    #[test]
    fn entry_from_tree_resolves_password_from_file_vars() {
        let conn = ProjectConnection {
            id: "pg".into(),
            label: "PG".into(),
            engine: "postgres".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Postgres {
                host: "localhost".into(),
                port: 5432,
                database: "db".into(),
                username: "u".into(),
                password: EnvOrString::FromEnv {
                    var: "BASED_PG_PASSWORD".into(),
                },
                ssl: false,
            },
            ssh: None,
        };
        let mut vars = HashMap::new();
        vars.insert("BASED_PG_PASSWORD".into(), "from-file".into());
        let entry = entry_from_tree(&conn, ConnectionOrigin::Project, &vars).unwrap();
        match entry.config {
            ConnectionConfig::Postgres(c) => assert_eq!(c.password, "from-file"),
            other => panic!("expected postgres, got {other:?}"),
        }
    }

    #[test]
    fn entry_from_tree_resolves_ssh_passphrase() {
        let conn = ProjectConnection {
            id: "pg".into(),
            label: "PG".into(),
            engine: "postgres".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Postgres {
                host: "mydb.internal".into(),
                port: 5432,
                database: "db".into(),
                username: "u".into(),
                password: EnvOrString::Literal(String::new()),
                ssl: true,
            },
            ssh: Some(SshSettings {
                host: "bastion.example.com".into(),
                port: 22,
                user: "ec2-user".into(),
                key_path: Some("~/.ssh/id_ed25519".into()),
                key_passphrase: Some(EnvOrString::FromEnv {
                    var: "BASED_PG_SSH_KEY_PASSPHRASE".into(),
                }),
            }),
        };
        let mut vars = HashMap::new();
        vars.insert("BASED_PG_SSH_KEY_PASSPHRASE".into(), "from-file".into());
        let entry = entry_from_tree(&conn, ConnectionOrigin::Project, &vars).unwrap();
        match entry.config {
            ConnectionConfig::Postgres(c) => {
                let ssh = c.ssh.expect("ssh");
                assert_eq!(ssh.host, "bastion.example.com");
                assert_eq!(ssh.key_passphrase.as_deref(), Some("from-file"));
            }
            other => panic!("expected postgres, got {other:?}"),
        }
    }
}
