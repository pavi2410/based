use std::collections::HashMap;

use based_core::EngineKind;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::{CollectionId, EnvironmentId, QueryId, TemplateId, WorkspaceId};

/// Workspace-level model (not repo-bound by default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceModel {
    pub id: WorkspaceId,
    pub name: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub active_environment_id: Option<EnvironmentId>,
    pub environments: Vec<Environment>,
    pub connection_templates: Vec<ConnectionTemplate>,
    pub collections: Vec<Collection>,
    pub loose_queries: Vec<WorkspaceQuery>,
}

impl WorkspaceModel {
    pub fn new(name: impl Into<String>) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: WorkspaceId::new(),
            name: name.into(),
            created_at: now,
            updated_at: now,
            active_environment_id: None,
            environments: Vec::new(),
            connection_templates: Vec::new(),
            collections: Vec::new(),
            loose_queries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub name: String,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTemplate {
    pub id: TemplateId,
    pub label: String,
    /// Defaults to Postgres for templates created before the engine column existed.
    #[serde(default = "legacy_template_engine")]
    pub engine: EngineKind,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: String,
}

/// A query saved in the workspace, loose or in a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceQuery {
    pub id: QueryId,
    pub name: String,
    pub sql: String,
    pub connection_template_id: Option<TemplateId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub queries: Vec<WorkspaceQuery>,
}

fn legacy_template_engine() -> EngineKind {
    EngineKind::Postgres
}
