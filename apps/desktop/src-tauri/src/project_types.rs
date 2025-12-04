use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: u32,
    pub name: String,
    pub description: Option<String>,
    pub connection: HashMap<String, ConnectionConfig>,
    pub settings: Option<ProjectSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Sqlite,
    #[serde(rename = "mongodb")]
    MongoDB,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretValue {
    Env { env: String },
    Value { value: String },
    File { file: String },
    Literal(String),
}

impl SecretValue {
    pub fn resolve(&self, env_vars: &HashMap<String, String>) -> Result<String, String> {
        match self {
            SecretValue::Env { env } => {
                env_vars
                    .get(env)
                    .cloned()
                    .or_else(|| std::env::var(env).ok())
                    .ok_or_else(|| format!("Environment variable not found: {}", env))
            }
            SecretValue::Value { value } => Ok(value.clone()),
            SecretValue::File { file } => {
                std::fs::read_to_string(file)
                    .map_err(|e| format!("Failed to read file {}: {}", file, e))
            }
            SecretValue::Literal(s) => Ok(s.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub label: Option<String>,
    pub engine: Engine,
    pub group: Option<String>,
    pub disabled: Option<bool>,
    pub order: Option<u32>,
    pub color: Option<String>,
    pub icon: Option<String>,

    // SQLite fields
    pub file: Option<String>,
    pub readonly: Option<bool>,

    // MongoDB fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<SecretValue>,

    // PostgreSQL fields
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<SecretValue>,
    pub ssl: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    #[serde(rename = "queryTimeout")]
    pub query_timeout: Option<u32>,
    #[serde(rename = "maxResultRows")]
    pub max_result_rows: Option<u32>,
    #[serde(rename = "enableQueryCache")]
    pub enable_query_cache: Option<bool>,
    #[serde(rename = "cacheTTL")]
    pub cache_ttl: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetadata {
    pub name: String,
    pub connection: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub parameters: Option<Vec<QueryParameter>>,
    pub favorite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: Option<serde_json::Value>,
    pub description: Option<String>,
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFile {
    pub path: String,
    pub name: String,
    pub connection: String,
    pub content: String,
    pub metadata: QueryMetadata,
}
