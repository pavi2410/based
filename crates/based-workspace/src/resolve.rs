use std::collections::HashMap;

use anyhow::{Context, Result, bail};

use crate::model::{ConnectionTemplate, WorkspaceModel};

#[derive(Debug, Clone)]
pub struct ResolvedConnectionTemplate {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: String,
}

/// Resolve a connection template against workspace/session/query scopes.
///
/// Scope precedence:
/// 1) session
/// 2) query
/// 3) collection
/// 4) environment (if active)
/// 5) workspace
pub fn resolve_connection_template(
    workspace: &WorkspaceModel,
    template: &ConnectionTemplate,
    workspace_vars: &HashMap<String, String>,
    collection_vars: &HashMap<String, String>,
    query_vars: &HashMap<String, String>,
    session_vars: &HashMap<String, String>,
) -> Result<ResolvedConnectionTemplate> {
    let env_vars = workspace
        .active_environment_id
        .and_then(|id| workspace.environments.iter().find(|e| e.id == id))
        .map(|e| e.variables.clone())
        .unwrap_or_default();

    let ctx = based_query::VariableContext {
        session: session_vars.clone(),
        query: query_vars.clone(),
        collection: collection_vars.clone(),
        environment: if workspace.active_environment_id.is_some() {
            Some(env_vars)
        } else {
            None
        },
        workspace: workspace_vars.clone(),
        connection: Default::default(),
    };

    let host = based_query::resolve_query(&template.host, &ctx)
        .map_err(|e| anyhow::anyhow!("host: {e}"))?;
    let port = based_query::resolve_query(&template.port, &ctx)
        .map_err(|e| anyhow::anyhow!("port: {e}"))?;
    let database = based_query::resolve_query(&template.database, &ctx)
        .map_err(|e| anyhow::anyhow!("database: {e}"))?;
    let username = based_query::resolve_query(&template.username, &ctx)
        .map_err(|e| anyhow::anyhow!("username: {e}"))?;
    let password = based_query::resolve_query(&template.password, &ctx)
        .map_err(|e| anyhow::anyhow!("password: {e}"))?;
    let ssl_mode = based_query::resolve_query(&template.ssl_mode, &ctx)
        .map_err(|e| anyhow::anyhow!("ssl_mode: {e}"))?;

    let port: u16 = port
        .parse()
        .with_context(|| format!("invalid port after variable resolution: {port}"))?;
    if port == 0 {
        bail!("port must be > 0");
    }

    Ok(ResolvedConnectionTemplate {
        host,
        port,
        database,
        username,
        password,
        ssl_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConnectionTemplate, Environment, WorkspaceModel};
    use uuid::Uuid;

    #[test]
    fn resolves_from_no_environment_workspace_scope() {
        let ws = WorkspaceModel::new("dev");
        let template = ConnectionTemplate {
            id: Uuid::new_v4(),
            label: "main".into(),
            host: "{{pg_host}}".into(),
            port: "{{pg_port}}".into(),
            database: "app".into(),
            username: "dev".into(),
            password: "secret".into(),
            ssl_mode: "prefer".into(),
        };
        let mut workspace_vars = HashMap::new();
        workspace_vars.insert("pg_host".into(), "localhost".into());
        workspace_vars.insert("pg_port".into(), "5432".into());

        let resolved = resolve_connection_template(
            &ws,
            &template,
            &workspace_vars,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(resolved.host, "localhost");
        assert_eq!(resolved.port, 5432);
    }

    #[test]
    fn session_overrides_environment() {
        let env_id = Uuid::new_v4();
        let mut ws = WorkspaceModel::new("dev");
        ws.active_environment_id = Some(env_id);
        ws.environments.push(Environment {
            id: env_id,
            name: "staging".into(),
            variables: HashMap::from([("pg_host".into(), "staging-db".into())]),
        });
        let template = ConnectionTemplate {
            id: Uuid::new_v4(),
            label: "main".into(),
            host: "{{pg_host}}".into(),
            port: "5432".into(),
            database: "app".into(),
            username: "dev".into(),
            password: "secret".into(),
            ssl_mode: "prefer".into(),
        };
        let session = HashMap::from([("pg_host".into(), "localhost".into())]);
        let resolved = resolve_connection_template(
            &ws,
            &template,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &session,
        )
        .unwrap();
        assert_eq!(resolved.host, "localhost");
    }
}
