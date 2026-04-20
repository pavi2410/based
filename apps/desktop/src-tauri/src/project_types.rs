use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProjectConfig {
    pub version: u32,
    pub name: String,
    pub description: Option<String>,
    pub connection: HashMap<String, ConnectionConfig>,
    pub settings: Option<ProjectSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Sqlite,
    #[serde(rename = "mongodb")]
    MongoDB,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

/// Saved query file (.query.toml) structure
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SavedQuery {
    /// Filename (not in TOML, added when reading)
    #[serde(skip)]
    pub filename: String,
    
    /// Display name
    pub name: String,
    /// Connection key from config.toml
    pub connection: String,
    /// Optional description
    pub description: Option<String>,
    /// Tags for organization
    pub tags: Option<Vec<String>>,
    /// Whether this query is favorited
    pub favorite: Option<bool>,
    
    /// Query parameters
    pub params: Option<std::collections::HashMap<String, QueryParameter>>,
    
    /// SQL query (for sqlite, postgres)
    pub sql: Option<SqlQuery>,
    /// MongoDB query
    pub mongo: Option<MongoQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SqlQuery {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MongoQuery {
    #[serde(rename = "type")]
    pub query_type: MongoQueryType,
    /// JSON string for find queries
    pub filter: Option<String>,
    /// JSON string for aggregation pipeline
    pub pipeline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum MongoQueryType {
    Find,
    Aggregate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QueryParameter {
    #[serde(rename = "type")]
    pub param_type: QueryParamType,
    pub default: Option<serde_json::Value>,
    pub description: Option<String>,
    /// Options for select type
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum QueryParamType {
    String,
    Number,
    Date,
    Boolean,
    Select,
}

/// Summary info for listing queries (without full content)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QuerySummary {
    pub filename: String,
    pub name: String,
    pub connection: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
}
