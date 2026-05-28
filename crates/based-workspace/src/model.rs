use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Workspace-level model (not repo-bound by default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceModel {
    pub id: Uuid,
    pub name: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub active_environment_id: Option<Uuid>,
    pub environments: Vec<Environment>,
    pub connection_templates: Vec<ConnectionTemplate>,
    pub collections: Vec<Collection>,
    pub loose_queries: Vec<LooseQuery>,
}

impl WorkspaceModel {
    pub fn new(name: impl Into<String>) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: Uuid::new_v4(),
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
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTemplate {
    pub id: Uuid,
    pub label: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LooseQuery {
    pub id: Uuid,
    pub name: String,
    pub sql: String,
    pub connection_template_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub queries: Vec<SavedQueryRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQueryRef {
    pub id: Uuid,
    pub name: String,
    pub sql: String,
    pub connection_template_id: Option<Uuid>,
}
