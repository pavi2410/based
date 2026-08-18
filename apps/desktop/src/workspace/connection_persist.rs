//! Write a wizard connection as `connections/*.toml` + `.env` in a based-dir.

use std::path::Path;

use based_project::{
    ConnectionSpec, EnvOrString, ProjectConnection, secret_env_key, slug_from_label,
    upsert_env_file, write_connection_file,
};

use crate::connection::ConnectionConfig;
use crate::postgres::SslMode;

pub fn persist_config_to_based_dir(
    based_dir: &Path,
    config: &ConnectionConfig,
    tags: &[String],
) -> anyhow::Result<ProjectConnection> {
    let relative_id = slug_from_label(config.label());
    persist_secret(based_dir, &relative_id, config)?;
    let conn = project_connection_from_config(config, relative_id, tags);
    write_connection_file(based_dir, &conn)?;
    Ok(conn)
}

fn persist_secret(
    based_dir: &Path,
    relative_id: &str,
    config: &ConnectionConfig,
) -> anyhow::Result<()> {
    let env_path = based_dir.join(".env");
    match config {
        ConnectionConfig::Postgres(c) if !c.password.is_empty() => {
            upsert_env_file(
                &env_path,
                &secret_env_key(relative_id, "PASSWORD"),
                &c.password,
            )?;
        }
        ConnectionConfig::MongoDB(c) if !c.uri.is_empty() => {
            upsert_env_file(&env_path, &secret_env_key(relative_id, "URL"), &c.uri)?;
        }
        _ => {}
    }
    Ok(())
}

fn project_connection_from_config(
    config: &ConnectionConfig,
    relative_id: String,
    tags: &[String],
) -> ProjectConnection {
    let tags = tags.to_vec();
    match config {
        ConnectionConfig::Postgres(c) => {
            let password = if c.password.is_empty() {
                EnvOrString::Literal(String::new())
            } else {
                EnvOrString::FromEnv {
                    var: secret_env_key(&relative_id, "PASSWORD"),
                }
            };
            ProjectConnection {
                id: relative_id,
                label: c.label.clone(),
                engine: "postgres".into(),
                tags,
                read_only: false,
                spec: ConnectionSpec::Postgres {
                    host: c.host.clone(),
                    port: c.port,
                    database: c.database.clone(),
                    username: c.username.clone(),
                    password,
                    ssl: !matches!(c.ssl_mode, SslMode::Disable),
                },
            }
        }
        ConnectionConfig::MongoDB(c) => {
            let url = if c.uri.is_empty() {
                EnvOrString::Literal(String::new())
            } else {
                EnvOrString::FromEnv {
                    var: secret_env_key(&relative_id, "URL"),
                }
            };
            ProjectConnection {
                id: relative_id,
                label: c.label.clone(),
                engine: "mongodb".into(),
                tags,
                read_only: false,
                spec: ConnectionSpec::MongoDB {
                    url,
                    database: c.database.clone(),
                },
            }
        }
        ConnectionConfig::SQLite(c) => ProjectConnection {
            id: relative_id,
            label: c.label.clone(),
            engine: "sqlite".into(),
            tags,
            read_only: c.read_only,
            spec: ConnectionSpec::Sqlite {
                file: c.path.clone(),
                pragma: None,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use based_project::{load_connections_from_based_dir, load_env_file};
    use std::path::PathBuf;

    use crate::postgres::PostgresConfig;
    use crate::sqlite::SqliteConfig;
    use std::fs;

    #[test]
    fn persist_postgres_writes_toml_and_env() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let config = ConnectionConfig::Postgres(PostgresConfig {
            label: "Analytics".into(),
            host: "db.example".into(),
            port: 6543,
            database: "analytics".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            ssl_mode: SslMode::Require,
        });
        let conn = persist_config_to_based_dir(based, &config, &[]).unwrap();
        assert_eq!(conn.id, "analytics");
        let loaded = load_connections_from_based_dir(based).unwrap();
        assert_eq!(loaded.len(), 1);
        let env = load_env_file(&based.join(".env")).unwrap();
        assert_eq!(
            env.get("BASED_ANALYTICS_PASSWORD").map(String::as_str),
            Some("s3cret")
        );
        let raw = fs::read_to_string(based.join("connections/analytics.toml")).unwrap();
        assert!(raw.contains("env = \"BASED_ANALYTICS_PASSWORD\""));
        assert!(!raw.contains("s3cret"));
    }

    #[test]
    fn persist_sqlite_does_not_write_env() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let config = ConnectionConfig::SQLite(SqliteConfig {
            label: "Northwind".into(),
            path: PathBuf::from("/tmp/northwind.db"),
            read_only: false,
            pragma: None,
        });
        persist_config_to_based_dir(based, &config, &[]).unwrap();
        assert!(!based.join(".env").exists());
        let loaded = load_connections_from_based_dir(based).unwrap();
        assert_eq!(loaded[0].id, "northwind");
    }

    #[test]
    fn persist_writes_connection_tags() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let config = ConnectionConfig::Postgres(PostgresConfig {
            label: "Analytics".into(),
            host: "db.example".into(),
            port: 5432,
            database: "analytics".into(),
            username: "alice".into(),
            password: String::new(),
            ssl_mode: SslMode::Disable,
        });
        persist_config_to_based_dir(based, &config, &["local".into(), "dev".into()]).unwrap();
        let loaded = load_connections_from_based_dir(based).unwrap();
        assert_eq!(loaded[0].tags, vec!["local", "dev"]);
        let raw = fs::read_to_string(based.join("connections/analytics.toml")).unwrap();
        assert!(raw.contains("local"));
        assert!(raw.contains("dev"));
    }
}
