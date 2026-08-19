use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::env_value::EnvOrString;
use crate::walk::{rel_id, walk_toml_files};

pub const CONNECTION_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub struct ProjectConnection {
    pub id: String,
    pub label: String,
    pub engine: String,
    pub tags: Vec<String>,
    pub read_only: bool,
    pub spec: ConnectionSpec,
    pub ssh: Option<SshSettings>,
}

/// Optional SSH hop persisted as `[ssh]` on a connection file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<String>,
    pub key_passphrase: Option<EnvOrString>,
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    #[serde(default)]
    ssh: Option<RawSsh>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawSsh {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key_passphrase: Option<EnvOrString>,
}

pub fn load_connections(project_root: &Path) -> Result<Vec<ProjectConnection>> {
    load_connections_from_based_dir(&project_root.join(".based"))
}

/// Load `connections/**/*.toml` from a based-dir (`<repo>/.based` or `~/.config/based`).
pub fn load_connections_from_based_dir(based_dir: &Path) -> Result<Vec<ProjectConnection>> {
    let dir = based_dir.join("connections");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let files = walk_toml_files(&dir)?;
    let mut connections = Vec::with_capacity(files.len());
    for path in files {
        connections.push(parse_connection_file(&dir, &path)?);
    }
    Ok(connections)
}

/// Stable file stem for a connection label (`Northwind` → `northwind`).
pub fn slug_from_label(label: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "connection".into()
    } else {
        out
    }
}

/// Write `connections/{id}.toml` under `based_dir`, creating parent folders as needed.
pub fn write_connection_file(based_dir: &Path, conn: &ProjectConnection) -> Result<PathBuf> {
    let rel = format!("{}.toml", conn.id);
    let path = based_dir.join("connections").join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let raw = WriteConnectionFile::from(conn);
    let content = toml::to_string_pretty(&raw)
        .with_context(|| format!("serialize connection {}", conn.id))?;
    fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[derive(Serialize)]
struct WriteConnectionFile {
    schema_version: u64,
    label: String,
    engine: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<EnvOrString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ssl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<EnvOrString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pragma: Option<PragmaSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ssh: Option<RawSsh>,
}

impl From<&ProjectConnection> for WriteConnectionFile {
    fn from(conn: &ProjectConnection) -> Self {
        let mut out = Self {
            schema_version: CONNECTION_SCHEMA_VERSION,
            label: conn.label.clone(),
            engine: conn.engine.clone(),
            tags: conn.tags.clone(),
            read_only: conn.read_only,
            file: None,
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            ssl: None,
            url: None,
            pragma: None,
            ssh: conn.ssh.as_ref().map(|s| RawSsh {
                host: Some(s.host.clone()),
                port: Some(s.port),
                user: Some(s.user.clone()),
                key_path: s.key_path.clone(),
                key_passphrase: s.key_passphrase.clone(),
            }),
        };
        match &conn.spec {
            ConnectionSpec::Sqlite { file, pragma } => {
                out.file = Some(file.display().to_string());
                out.pragma = pragma.clone();
            }
            ConnectionSpec::Postgres {
                host,
                port,
                database,
                username,
                password,
                ssl,
            } => {
                out.host = Some(host.clone());
                out.port = Some(*port);
                out.database = Some(database.clone());
                out.username = Some(username.clone());
                out.password = Some(password.clone());
                out.ssl = Some(*ssl);
            }
            ConnectionSpec::MongoDB { url, database } => {
                out.url = Some(url.clone());
                out.database = database.clone();
            }
        }
        out
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn parse_connection_file(connections_dir: &Path, path: &Path) -> Result<ProjectConnection> {
    let rel = path
        .strip_prefix(connections_dir)
        .with_context(|| format!("connection path not under {}", connections_dir.display()))?;
    let id = rel_id(rel);
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
    let ssh = parse_ssh(file.ssh, &id)?;
    Ok(ProjectConnection {
        id,
        label: file.label,
        engine,
        tags: file.tags,
        read_only,
        spec,
        ssh,
    })
}

fn parse_ssh(raw: Option<RawSsh>, connection_id: &str) -> Result<Option<SshSettings>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let host = raw.host.unwrap_or_default();
    let user = raw.user.unwrap_or_default();
    if host.trim().is_empty() || user.trim().is_empty() {
        bail!("connection {connection_id} [ssh] requires `host` and `user`");
    }
    Ok(Some(SshSettings {
        host,
        port: raw.port.unwrap_or(22),
        user,
        key_path: raw.key_path.filter(|p| !p.trim().is_empty()),
        key_passphrase: raw.key_passphrase,
    }))
}

fn resolve_read_only(explicit: Option<bool>, tags: &[String]) -> bool {
    if let Some(v) = explicit {
        return v;
    }
    tags.iter().any(|t| t.eq_ignore_ascii_case("readonly"))
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
    fn connection_id_from_plain_and_legacy_toml() {
        assert_eq!(rel_id(Path::new("local/northwind.toml")), "local/northwind");
        assert_eq!(
            rel_id(Path::new("local/northwind.conn.toml")),
            "local/northwind"
        );
    }

    #[test]
    fn parse_sqlite_read_only_from_toml() {
        let dir = tempfile::tempdir().unwrap();
        let conn_dir = dir.path().join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        let path = conn_dir.join("index.toml");
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

    #[test]
    fn slug_from_label_lowercases_and_hyphenates() {
        assert_eq!(slug_from_label("Northwind"), "northwind");
        assert_eq!(
            slug_from_label("Local PostgreSQL (Docker)"),
            "local-postgresql-docker"
        );
        assert_eq!(slug_from_label("  "), "connection");
    }

    #[test]
    fn load_connections_from_based_dir_does_not_require_a_parent_project() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let conn_dir = based.join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        fs::write(
            conn_dir.join("personal.toml"),
            r#"
schema_version = 1
label = "Personal PG"
engine = "postgres"
host = "localhost"
port = 5432
database = "app"
username = "me"
password = ""
ssl = false
"#,
        )
        .unwrap();
        let loaded = load_connections_from_based_dir(based).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "personal");
        assert_eq!(loaded[0].label, "Personal PG");
    }

    #[test]
    fn parse_postgres_ssh_table_defaults_port_22() {
        let dir = tempfile::tempdir().unwrap();
        let conn_dir = dir.path().join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        let path = conn_dir.join("prod.toml");
        fs::write(
            &path,
            r#"
schema_version = 1
label = "Prod"
engine = "postgres"
host = "mydb.internal"
port = 5432
database = "app"
username = "app"
password = ""
ssl = true

[ssh]
host = "bastion.example.com"
user = "ec2-user"
key_path = "~/.ssh/id_ed25519"
"#,
        )
        .unwrap();
        let conn = parse_connection_file(&conn_dir, &path).unwrap();
        let ssh = conn.ssh.expect("ssh table");
        assert_eq!(ssh.host, "bastion.example.com");
        assert_eq!(ssh.port, 22);
        assert_eq!(ssh.user, "ec2-user");
        assert_eq!(ssh.key_path.as_deref(), Some("~/.ssh/id_ed25519"));
        assert!(ssh.key_passphrase.is_none());
    }

    #[test]
    fn empty_ssh_table_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let conn_dir = dir.path().join("connections");
        fs::create_dir_all(&conn_dir).unwrap();
        let path = conn_dir.join("bad.toml");
        fs::write(
            &path,
            r#"
schema_version = 1
label = "Prod"
engine = "postgres"
host = "mydb.internal"
port = 5432
database = "app"
username = "app"
password = ""
ssl = false

[ssh]
"#,
        )
        .unwrap();
        let err = parse_connection_file(&conn_dir, &path).unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("ssh"),
            "expected ssh validation error, got {err}"
        );
    }

    #[test]
    fn write_then_load_ssh_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let conn = ProjectConnection {
            id: "prod".into(),
            label: "Prod".into(),
            engine: "postgres".into(),
            tags: vec![],
            read_only: false,
            spec: ConnectionSpec::Postgres {
                host: "mydb.internal".into(),
                port: 5432,
                database: "app".into(),
                username: "app".into(),
                password: EnvOrString::Literal(String::new()),
                ssl: true,
            },
            ssh: Some(SshSettings {
                host: "bastion.example.com".into(),
                port: 22,
                user: "ec2-user".into(),
                key_path: Some("~/.ssh/id_ed25519".into()),
                key_passphrase: Some(EnvOrString::FromEnv {
                    var: "BASED_PROD_SSH_KEY_PASSPHRASE".into(),
                }),
            }),
        };
        write_connection_file(based, &conn).unwrap();
        let raw = fs::read_to_string(based.join("connections/prod.toml")).unwrap();
        assert!(raw.contains("[ssh]"));
        assert!(raw.contains("bastion.example.com"));
        assert!(raw.contains("BASED_PROD_SSH_KEY_PASSPHRASE"));
        assert!(!raw.contains("s3cret"));
        let loaded = load_connections_from_based_dir(based).unwrap();
        let ssh = loaded[0].ssh.as_ref().expect("ssh");
        assert_eq!(ssh.user, "ec2-user");
        assert_eq!(
            ssh.key_passphrase,
            Some(EnvOrString::FromEnv {
                var: "BASED_PROD_SSH_KEY_PASSPHRASE".into(),
            })
        );
    }

    #[test]
    fn write_then_load_postgres_and_sqlite_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let based = dir.path();
        let postgres = ProjectConnection {
            id: "analytics".into(),
            label: "Analytics".into(),
            engine: "postgres".into(),
            tags: vec!["local".into()],
            read_only: false,
            spec: ConnectionSpec::Postgres {
                host: "db.example".into(),
                port: 6543,
                database: "analytics".into(),
                username: "alice".into(),
                password: EnvOrString::FromEnv {
                    var: "BASED_ANALYTICS_PASSWORD".into(),
                },
                ssl: true,
            },
            ssh: None,
        };
        let sqlite = ProjectConnection {
            id: "northwind".into(),
            label: "Northwind".into(),
            engine: "sqlite".into(),
            tags: vec![],
            read_only: true,
            spec: ConnectionSpec::Sqlite {
                file: PathBuf::from("/tmp/northwind.db"),
                pragma: None,
            },
            ssh: None,
        };
        let pg_path = write_connection_file(based, &postgres).unwrap();
        let sqlite_path = write_connection_file(based, &sqlite).unwrap();
        assert_eq!(pg_path, based.join("connections").join("analytics.toml"));
        assert_eq!(
            sqlite_path,
            based.join("connections").join("northwind.toml")
        );
        let raw = fs::read_to_string(&pg_path).unwrap();
        assert!(raw.contains("env = \"BASED_ANALYTICS_PASSWORD\""));
        assert!(!raw.contains("s3cret"));

        let loaded = load_connections_from_based_dir(based).unwrap();
        assert_eq!(loaded.len(), 2);
        let pg = loaded.iter().find(|c| c.id == "analytics").unwrap();
        assert_eq!(pg.label, "Analytics");
        match &pg.spec {
            ConnectionSpec::Postgres {
                host,
                port,
                database,
                username,
                password,
                ssl,
            } => {
                assert_eq!(host, "db.example");
                assert_eq!(port, &6543);
                assert_eq!(database, "analytics");
                assert_eq!(username, "alice");
                assert_eq!(
                    password,
                    &EnvOrString::FromEnv {
                        var: "BASED_ANALYTICS_PASSWORD".into()
                    }
                );
                assert!(ssl);
            }
            other => panic!("expected postgres, got {other:?}"),
        }
        let lite = loaded.iter().find(|c| c.id == "northwind").unwrap();
        assert!(lite.read_only);
        match &lite.spec {
            ConnectionSpec::Sqlite { file, .. } => {
                assert_eq!(file.as_path(), Path::new("/tmp/northwind.db"));
            }
            other => panic!("expected sqlite, got {other:?}"),
        }
    }
}
