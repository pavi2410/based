use std::collections::HashMap;
use std::path::Path;

use based_core::{Secret, SshTunnelConfig};
use based_project::{
    ConnectionSpec, MissingEnvVar, PragmaSettings, ProjectConnection, SshSettings,
    load_connections_from_based_dir, load_env_file,
};
use based_sqlite::SqlitePragma;

use crate::connection::{ConnectionConfig, ConnectionEntry, ConnectionId, ConnectionOrigin};
use crate::mongodb::MongoConfig;
use crate::postgres::PostgresConfig;
use crate::redis::RedisConfig;
use crate::sqlite::SqliteConfig;

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
            ssl_mode,
        } => ConnectionConfig::Postgres(PostgresConfig {
            label: conn.label.clone(),
            host: host.clone(),
            port: *port,
            database: database.clone(),
            username: username.clone(),
            password: Secret::new(password.resolve_with(file_vars)?),
            ssl_mode: *ssl_mode,
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
        ConnectionSpec::Redis { url } => {
            let url = url.resolve_with(file_vars)?;
            if url.trim().is_empty() {
                anyhow::bail!("redis connection {} has empty url", conn.id);
            }
            ConnectionConfig::Redis(RedisConfig {
                label: conn.label.clone(),
                url,
            })
        }
    };
    let id = match origin {
        ConnectionOrigin::Project => ConnectionId::project(&conn.id),
        ConnectionOrigin::Personal => ConnectionId::personal(conn.id.as_str()),
    };
    let mut entry = ConnectionEntry::new(id, config, conn.tags.clone());
    entry.ai = conn.ai;
    Ok(entry)
}

/// A connection file that did not load, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedConnection {
    /// Connection id, or the based-dir when the whole folder failed to parse.
    pub id: String,
    /// The `.env` variable it needs, when that is the reason.
    pub missing_var: Option<String>,
    pub reason: String,
}

/// Connections from a based-dir, plus the ones that could not load.
#[derive(Default)]
pub struct LoadedEntries {
    pub entries: Vec<ConnectionEntry>,
    pub skipped: Vec<SkippedConnection>,
}

pub fn load_entries_from_based_dir(based_dir: &Path, origin: ConnectionOrigin) -> LoadedEntries {
    let file_vars = load_env_file(&based_dir.join(".env")).unwrap_or_default();
    let connections = match load_connections_from_based_dir(based_dir) {
        Ok(c) => c,
        Err(err) => {
            log::warn!(
                "load connections from {} failed: {err:#}",
                based_dir.display()
            );
            return LoadedEntries {
                entries: Vec::new(),
                skipped: vec![SkippedConnection {
                    id: based_dir.display().to_string(),
                    missing_var: None,
                    reason: format!("{err:#}"),
                }],
            };
        }
    };
    let mut loaded = LoadedEntries::default();
    for conn in &connections {
        match entry_from_tree(conn, origin, &file_vars) {
            Ok(entry) => loaded.entries.push(entry),
            Err(err) => {
                log::warn!("connection {} skipped: {err:#}", conn.id);
                let missing_var = err
                    .chain()
                    .find_map(|e| e.downcast_ref::<MissingEnvVar>())
                    .map(|m| m.var.clone());
                loaded.skipped.push(SkippedConnection {
                    id: conn.id.to_string(),
                    missing_var,
                    reason: format!("{err:#}"),
                });
            }
        }
    }
    loaded
}

fn resolve_ssh(
    ssh: &SshSettings,
    file_vars: &HashMap<String, String>,
) -> anyhow::Result<SshTunnelConfig> {
    Ok(SshTunnelConfig {
        endpoint: ssh.endpoint.clone(),
        key_passphrase: ssh
            .key_passphrase
            .as_ref()
            .map(|v| v.resolve_with(file_vars).map(Secret::new))
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
    use based_core::SshEndpoint;
    use based_project::{EnvOrString, write_connection_file};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn personal_tree_entry_uses_user_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let conn = ProjectConnection {
            id: "analytics".into(),
            label: "Analytics".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Sqlite {
                file: PathBuf::from("/tmp/a.db"),
                pragma: None,
            },
            ssh: None,
            ai: based_core::ContextLevel::Off,
        };
        write_connection_file(based, &conn).unwrap();
        let entries = load_entries_from_based_dir(based, ConnectionOrigin::Personal).entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, ConnectionId::personal("analytics"));
        assert_eq!(entries[0].origin(), ConnectionOrigin::Personal);
    }

    #[test]
    fn connections_missing_a_secret_are_reported_not_dropped_silently() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let conn_dir = based.join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        fs::write(
            conn_dir.join("prod.toml"),
            r#"
schema_version = 1
label = "Prod"
engine = "postgres"
host = "db"
database = "app"
username = "app"
password = { env = "BASED_TEST_LOADER_MISSING_SECRET" }
"#,
        )
        .unwrap();
        fs::write(
            conn_dir.join("local.toml"),
            "schema_version = 1\nlabel = \"Local\"\nengine = \"sqlite\"\nfile = \"/tmp/a.db\"\n",
        )
        .unwrap();
        let loaded = load_entries_from_based_dir(based, ConnectionOrigin::Project);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].id, "prod");
        assert_eq!(
            loaded.skipped[0].missing_var.as_deref(),
            Some("BASED_TEST_LOADER_MISSING_SECRET")
        );
    }

    #[test]
    fn entry_from_tree_resolves_password_from_file_vars() {
        let conn = ProjectConnection {
            id: "pg".into(),
            label: "PG".into(),
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
                ssl_mode: based_core::SslMode::Disable,
            },
            ssh: None,
            ai: based_core::ContextLevel::Off,
        };
        let mut vars = HashMap::new();
        vars.insert("BASED_PG_PASSWORD".into(), "from-file".into());
        let entry = entry_from_tree(&conn, ConnectionOrigin::Project, &vars).unwrap();
        match entry.config {
            ConnectionConfig::Postgres(c) => assert_eq!(c.password.expose(), "from-file"),
            other => panic!("expected postgres, got {other:?}"),
        }
    }

    #[test]
    fn entry_from_tree_resolves_ssh_passphrase() {
        let conn = ProjectConnection {
            id: "pg".into(),
            label: "PG".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Postgres {
                host: "mydb.internal".into(),
                port: 5432,
                database: "db".into(),
                username: "u".into(),
                password: EnvOrString::Literal(String::new()),
                ssl_mode: based_core::SslMode::Require,
            },
            ssh: Some(SshSettings {
                endpoint: SshEndpoint {
                    host: "bastion.example.com".into(),
                    port: 22,
                    user: "ec2-user".into(),
                    key_path: Some("~/.ssh/id_ed25519".into()),
                },
                key_passphrase: Some(EnvOrString::FromEnv {
                    var: "BASED_PG_SSH_KEY_PASSPHRASE".into(),
                }),
            }),
            ai: based_core::ContextLevel::Off,
        };
        let mut vars = HashMap::new();
        vars.insert("BASED_PG_SSH_KEY_PASSPHRASE".into(), "from-file".into());
        let entry = entry_from_tree(&conn, ConnectionOrigin::Project, &vars).unwrap();
        match entry.config {
            ConnectionConfig::Postgres(c) => {
                let ssh = c.ssh.expect("ssh");
                assert_eq!(ssh.endpoint.host, "bastion.example.com");
                assert_eq!(
                    ssh.key_passphrase.as_ref().map(Secret::expose),
                    Some("from-file")
                );
            }
            other => panic!("expected postgres, got {other:?}"),
        }
    }
}
