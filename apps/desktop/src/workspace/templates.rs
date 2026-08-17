//! Bind workspace connection templates to the connection registry.

use std::collections::HashMap;
use std::path::PathBuf;

use based_core::EngineKind;
use based_workspace::{ConnectionTemplate, WorkspaceModel, resolve_connection_template};
use uuid::Uuid;

use crate::connection::{ConnectionConfig, ConnectionEntry};
use crate::mongodb::MongoConfig;
use crate::postgres::{PostgresConfig, SslMode};
use crate::sqlite::SqliteConfig;

const TEMPLATE_KEY_PREFIX: &str = "ws-template:";

pub fn template_stable_key(template_id: Uuid) -> String {
    format!("{TEMPLATE_KEY_PREFIX}{template_id}")
}

pub fn is_template_key(stable_key: &str) -> bool {
    stable_key.starts_with(TEMPLATE_KEY_PREFIX)
}

/// Build a persistable template and registry entry for a wizard Connect.
///
/// Reuses an existing template id when the workspace already has the same
/// label+engine so later persist does not create a duplicate sidebar row.
pub fn entry_from_wizard_config(
    workspace: &WorkspaceModel,
    config: &ConnectionConfig,
) -> (ConnectionTemplate, ConnectionEntry) {
    let existing_id = workspace
        .connection_templates
        .iter()
        .find(|t| t.label == config.label() && t.engine == config.engine())
        .map(|t| t.id);
    let template = template_from_config(config, existing_id);
    let entry = resolve_template_entry(workspace, &template).unwrap_or_else(|_| {
        ConnectionEntry::with_stable_id(config.clone(), &template_stable_key(template.id))
    });
    (template, entry)
}

pub fn template_from_config(
    config: &ConnectionConfig,
    existing_id: Option<Uuid>,
) -> ConnectionTemplate {
    let id = existing_id.unwrap_or_else(Uuid::new_v4);
    match config {
        ConnectionConfig::Postgres(c) => ConnectionTemplate {
            id,
            label: c.label.clone(),
            engine: EngineKind::Postgres,
            host: c.host.clone(),
            port: c.port.to_string(),
            database: c.database.clone(),
            username: c.username.clone(),
            password: c.password.clone(),
            ssl_mode: ssl_mode_label(c.ssl_mode).to_string(),
        },
        ConnectionConfig::MongoDB(c) => ConnectionTemplate {
            id,
            label: c.label.clone(),
            engine: EngineKind::MongoDB,
            host: c.uri.clone(),
            port: "27017".into(),
            database: c.database.clone().unwrap_or_default(),
            username: c.auth_source.clone().unwrap_or_default(),
            password: String::new(),
            ssl_mode: String::new(),
        },
        ConnectionConfig::SQLite(c) => ConnectionTemplate {
            id,
            label: c.label.clone(),
            engine: EngineKind::SQLite,
            host: c.path.display().to_string(),
            port: "1".into(),
            database: String::new(),
            username: String::new(),
            password: String::new(),
            ssl_mode: String::new(),
        },
    }
}

pub fn template_from_postgres_config(
    config: &PostgresConfig,
    existing_id: Option<Uuid>,
) -> ConnectionTemplate {
    template_from_config(&ConnectionConfig::Postgres(config.clone()), existing_id)
}

pub fn resolve_template_entry(
    workspace: &WorkspaceModel,
    template: &ConnectionTemplate,
) -> anyhow::Result<ConnectionEntry> {
    let workspace_vars = HashMap::new();
    let resolved = resolve_connection_template(
        workspace,
        template,
        &workspace_vars,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )?;

    let config = match template.engine {
        EngineKind::Postgres => {
            let ssl_mode = parse_ssl_mode(&resolved.ssl_mode);
            ConnectionConfig::Postgres(PostgresConfig {
                label: template.label.clone(),
                host: resolved.host,
                port: resolved.port,
                database: resolved.database,
                username: resolved.username,
                password: resolved.password,
                ssl_mode,
            })
        }
        EngineKind::MongoDB => ConnectionConfig::MongoDB(MongoConfig {
            label: template.label.clone(),
            uri: resolved.host,
            database: nonempty_opt(resolved.database),
            auth_source: nonempty_opt(resolved.username),
        }),
        EngineKind::SQLite => ConnectionConfig::SQLite(SqliteConfig {
            label: template.label.clone(),
            path: PathBuf::from(resolved.host),
            read_only: false,
            pragma: None,
        }),
    };

    Ok(ConnectionEntry::with_stable_id(
        config,
        &template_stable_key(template.id),
    ))
}

pub fn entries_from_workspace(workspace: &WorkspaceModel) -> Vec<ConnectionEntry> {
    workspace
        .connection_templates
        .iter()
        .filter_map(|t| resolve_template_entry(workspace, t).ok())
        .collect()
}

fn nonempty_opt(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn ssl_mode_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::Disable => "disable",
        SslMode::Prefer => "prefer",
        SslMode::Require => "require",
        SslMode::VerifyCa | SslMode::VerifyFull => "verify-full",
    }
}

fn parse_ssl_mode(raw: &str) -> SslMode {
    match raw.to_ascii_lowercase().as_str() {
        "disable" | "off" | "false" => SslMode::Disable,
        "require" | "on" | "true" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => SslMode::Prefer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use based_workspace::WorkspaceModel;

    #[test]
    fn template_key_prefix_is_stable() {
        let id = Uuid::nil();
        assert!(is_template_key(&template_stable_key(id)));
    }

    #[test]
    fn postgres_config_round_trips() {
        let ws = WorkspaceModel::new("test");
        let config = ConnectionConfig::Postgres(PostgresConfig {
            label: "Local PG".into(),
            host: "db.example".into(),
            port: 6543,
            database: "analytics".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            ssl_mode: SslMode::Require,
        });
        let template = template_from_config(&config, None);
        assert_eq!(template.engine, EngineKind::Postgres);
        let entry = resolve_template_entry(&ws, &template).expect("resolve");
        match entry.config {
            ConnectionConfig::Postgres(c) => {
                assert_eq!(c.label, "Local PG");
                assert_eq!(c.host, "db.example");
                assert_eq!(c.port, 6543);
                assert_eq!(c.database, "analytics");
                assert_eq!(c.username, "alice");
                assert_eq!(c.password, "s3cret");
                assert!(matches!(c.ssl_mode, SslMode::Require));
            }
            other => panic!("expected Postgres, got {other:?}"),
        }
    }

    #[test]
    fn mongo_config_round_trips() {
        let ws = WorkspaceModel::new("test");
        let config = ConnectionConfig::MongoDB(MongoConfig {
            label: "Local Mongo".into(),
            uri: "mongodb://127.0.0.1:27017".into(),
            database: Some("app".into()),
            auth_source: Some("admin".into()),
        });
        let template = template_from_config(&config, None);
        assert_eq!(template.engine, EngineKind::MongoDB);
        assert_eq!(template.host, "mongodb://127.0.0.1:27017");
        assert_eq!(template.port, "27017");
        let entry = resolve_template_entry(&ws, &template).expect("resolve");
        match entry.config {
            ConnectionConfig::MongoDB(c) => {
                assert_eq!(c.label, "Local Mongo");
                assert_eq!(c.uri, "mongodb://127.0.0.1:27017");
                assert_eq!(c.database.as_deref(), Some("app"));
                assert_eq!(c.auth_source.as_deref(), Some("admin"));
            }
            other => panic!("expected MongoDB, got {other:?}"),
        }
    }

    #[test]
    fn wizard_entry_reuses_existing_template_id() {
        let mut ws = WorkspaceModel::new("test");
        let config = ConnectionConfig::Postgres(PostgresConfig {
            label: "Local PG".into(),
            host: "localhost".into(),
            port: 5432,
            database: "postgres".into(),
            username: "postgres".into(),
            password: String::new(),
            ssl_mode: SslMode::Prefer,
        });
        let existing = template_from_config(&config, None);
        ws.connection_templates.push(existing.clone());

        let (template, entry) = entry_from_wizard_config(&ws, &config);
        assert_eq!(template.id, existing.id);
        assert_eq!(
            entry.id,
            crate::connection::ConnectionId::from_key(&template_stable_key(existing.id))
        );
        assert!(matches!(
            entry.state,
            crate::connection::ConnectionState::Disconnected
        ));
    }

    #[test]
    fn sqlite_config_round_trips() {
        let ws = WorkspaceModel::new("test");
        let config = ConnectionConfig::SQLite(SqliteConfig {
            label: "Northwind".into(),
            path: PathBuf::from("/tmp/northwind.db"),
            read_only: false,
            pragma: None,
        });
        let template = template_from_config(&config, None);
        assert_eq!(template.engine, EngineKind::SQLite);
        assert_eq!(template.host, "/tmp/northwind.db");
        assert_eq!(template.port, "1");
        let entry = resolve_template_entry(&ws, &template).expect("resolve");
        match entry.config {
            ConnectionConfig::SQLite(c) => {
                assert_eq!(c.label, "Northwind");
                assert_eq!(c.path, PathBuf::from("/tmp/northwind.db"));
            }
            other => panic!("expected SQLite, got {other:?}"),
        }
    }
}
