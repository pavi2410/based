//! Bind workspace connection templates to the connection registry.

use std::collections::HashMap;

use based_workspace::{ConnectionTemplate, WorkspaceModel, resolve_connection_template};
use uuid::Uuid;

use crate::connection::{ConnectionConfig, ConnectionEntry};
use crate::postgres::{PostgresConfig, SslMode};

const TEMPLATE_KEY_PREFIX: &str = "ws-template:";

pub fn template_stable_key(template_id: Uuid) -> String {
    format!("{TEMPLATE_KEY_PREFIX}{template_id}")
}

pub fn is_template_key(stable_key: &str) -> bool {
    stable_key.starts_with(TEMPLATE_KEY_PREFIX)
}

pub fn template_from_postgres_config(
    config: &PostgresConfig,
    existing_id: Option<Uuid>,
) -> ConnectionTemplate {
    ConnectionTemplate {
        id: existing_id.unwrap_or_else(Uuid::new_v4),
        label: config.label.clone(),
        host: config.host.clone(),
        port: config.port.to_string(),
        database: config.database.clone(),
        username: config.username.clone(),
        password: config.password.clone(),
        ssl_mode: ssl_mode_label(config.ssl_mode).to_string(),
    }
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
    let ssl_mode = parse_ssl_mode(&resolved.ssl_mode);
    let config = ConnectionConfig::Postgres(PostgresConfig {
        label: template.label.clone(),
        host: resolved.host,
        port: resolved.port,
        database: resolved.database,
        username: resolved.username,
        password: resolved.password,
        ssl_mode,
    });
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
        _ => SslMode::Prefer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_key_prefix_is_stable() {
        let id = Uuid::nil();
        assert!(is_template_key(&template_stable_key(id)));
    }
}
