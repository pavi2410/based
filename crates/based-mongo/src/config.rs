use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub label: String,
    pub uri: String,
    pub database: Option<String>,
    pub auth_source: Option<String>,
}
