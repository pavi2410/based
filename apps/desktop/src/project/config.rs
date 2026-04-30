// .based/config.toml read/write.
//
// Format:
// [project]
// name = "my project"
//
// [[connections]]
// engine = "postgres"
// label = "prod-db"
// host = "localhost"
// port = 5432
// database = "mydb"
// user = "postgres"
//
// [[connections]]
// engine = "sqlite"
// label = "app.db"
// path = "/path/to/app.db"
//
// [[connections]]
// engine = "mongodb"
// label = "local-mongo"
// uri = "mongodb://localhost:27017"
// database = "mydb"

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    #[serde(default)]
    pub connections: Vec<ConnectionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMeta {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum ConnectionDef {
    Postgres(PostgresDef),
    Sqlite(SqliteDef),
    Mongodb(MongoDbDef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresDef {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    #[serde(default)]
    pub ssl_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteDef {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoDbDef {
    pub label: String,
    pub uri: String,
    pub database: String,
}

impl ProjectConfig {
    pub fn load(project_dir: &Path) -> Result<Self> {
        let path = Self::config_path(project_dir);
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, project_dir: &Path) -> Result<()> {
        let path = Self::config_path(project_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn config_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".based").join("config.toml")
    }
}
