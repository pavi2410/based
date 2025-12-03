use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: String,
    pub name: String,
    pub description: Option<String>,
    pub databases: HashMap<String, DatabaseConfig>,
    pub environments: EnvironmentConfig,
    pub settings: Option<ProjectSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub db_type: DatabaseType,
    pub connection: ConnectionConfig,
    pub description: Option<String>,
    pub environments: Option<HashMap<String, DatabaseConfigOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfigOverride {
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Sqlite,
    MongoDB,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    // SQLite
    pub path: Option<String>,

    // MongoDB
    pub url: Option<String>,

    // PostgreSQL
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub sslmode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub default: String,
    pub available: Vec<String>,
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
    pub database: String,
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
    pub database: String,
    pub content: String,
    pub metadata: QueryMetadata,
}
