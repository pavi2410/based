use crate::project_types::*;
use crate::variables::VariableError;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tauri::command;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParseError(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerializeError(#[from] toml::ser::Error),
    #[error("YAML parse error: {0}")]
    YamlParseError(#[from] serde_yaml::Error),
    #[error("Variable resolution error: {0}")]
    VariableError(#[from] VariableError),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),
    #[error("Project not initialized at: {0}")]
    ProjectNotInitialized(String),
    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
}

/// Initialize a new Based project in the given directory
#[command]
pub async fn initialize_project(project_path: String) -> Result<(), String> {
    let based_dir = Path::new(&project_path).join(".based");

    // Create directory structure
    fs::create_dir_all(based_dir.join("queries"))
        .map_err(|e| format!("Failed to create queries directory: {}", e))?;
    fs::create_dir_all(based_dir.join("templates"))
        .map_err(|e| format!("Failed to create templates directory: {}", e))?;
    fs::create_dir_all(based_dir.join(".local/cache"))
        .map_err(|e| format!("Failed to create .local/cache directory: {}", e))?;

    // Create default config.toml
    let default_config = ProjectConfig {
        version: 1,
        name: Path::new(&project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("New Project")
            .to_string(),
        description: Some("A new Based project".to_string()),
        connection: HashMap::new(),
        settings: Some(ProjectSettings {
            query_timeout: Some(30000),
            max_result_rows: Some(1000),
            enable_query_cache: Some(true),
            cache_ttl: Some(3600),
        }),
    };

    let config_content = toml::to_string_pretty(&default_config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(based_dir.join("config.toml"), config_content)
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    // Create .gitignore
    let gitignore_content = r#".local/
*.local.*
.env
.env.local
cache/
*.cache
.DS_Store
Thumbs.db
"#;
    fs::write(based_dir.join(".gitignore"), gitignore_content)
        .map_err(|e| format!("Failed to write .gitignore: {}", e))?;

    // Create .env.example
    let env_example = r#"# Example environment variables
# Copy to .env and fill in values

# Example PostgreSQL connection
# PROD_DB_HOST=prod-db.example.com
# PROD_DB_USER=admin
# PROD_DB_PASSWORD=secret

# Example MongoDB connection
# MONGO_URL=mongodb://localhost:27017/mydb
"#;
    fs::write(based_dir.join(".env.example"), env_example)
        .map_err(|e| format!("Failed to write .env.example: {}", e))?;

    Ok(())
}

/// Read and parse project config
#[command]
pub async fn read_project_config(project_path: String) -> Result<ProjectConfig, String> {
    let config_path = Path::new(&project_path).join(".based/config.toml");

    if !config_path.exists() {
        return Err(format!("Project not initialized at: {}", project_path));
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;

    let config: ProjectConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config.toml: {}", e))?;

    Ok(config)
}

/// Write project config
#[command]
pub async fn write_project_config(
    project_path: String,
    config: ProjectConfig,
) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".based/config.toml");

    let content = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    Ok(())
}

/// List all saved queries in the project with summary info
#[command]
pub async fn list_saved_queries(project_path: String) -> Result<Vec<QuerySummary>, String> {
    let queries_path = Path::new(&project_path).join(".based/queries");

    if !queries_path.exists() {
        return Ok(Vec::new());
    }

    let mut queries = Vec::new();

    for entry in WalkDir::new(&queries_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            
            // Only include .query.toml files
            if filename.ends_with(".query.toml") {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(query) = toml::from_str::<SavedQuery>(&content) {
                        queries.push(QuerySummary {
                            filename: filename.to_string(),
                            name: query.name,
                            connection: query.connection,
                            description: query.description,
                            tags: query.tags,
                            favorite: query.favorite,
                        });
                    }
                }
            }
        }
    }

    // Sort by name
    queries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(queries)
}

/// Read a saved query file
#[command]
pub async fn get_saved_query(
    project_path: String,
    filename: String,
) -> Result<SavedQuery, String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&filename);

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read query file: {}", e))?;

    let mut query: SavedQuery = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse query file: {}", e))?;
    
    // Set filename (not in TOML)
    query.filename = filename;

    Ok(query)
}

/// Save a query file (create or update)
#[command]
pub async fn save_query(
    project_path: String,
    filename: String,
    query: SavedQuery,
) -> Result<(), String> {
    // Validate filename
    if filename.is_empty() {
        return Err("Filename cannot be empty".to_string());
    }
    if !filename.ends_with(".query.toml") {
        return Err("Filename must end with .query.toml".to_string());
    }
    
    let queries_dir = Path::new(&project_path).join(".based/queries");
    
    // Ensure queries directory exists
    fs::create_dir_all(&queries_dir)
        .map_err(|e| format!("Failed to create queries directory: {}", e))?;
    
    let file_path = queries_dir.join(&filename);
    
    // Make sure we're not writing to a directory
    if file_path.is_dir() {
        return Err(format!("Cannot write to directory: {}", file_path.display()));
    }

    // Serialize to TOML
    let content = toml::to_string_pretty(&query)
        .map_err(|e| format!("Failed to serialize query: {}", e))?;

    fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write query file '{}': {}", filename, e))?;

    Ok(())
}

/// Delete a saved query file
#[command]
pub async fn delete_saved_query(
    project_path: String,
    filename: String,
) -> Result<(), String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&filename);

    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete query file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_query_toml_parsing() {
        let content = r#"
name = "Recent Orders"
connection = "northwind"
description = "Get orders from the last N days"
tags = ["orders", "reports"]
favorite = true

[params.days]
type = "number"
default = 7
description = "Number of days to look back"

[sql]
query = "SELECT * FROM orders WHERE order_date > date('now', '-' || $days || ' days')"
"#;

        let query: SavedQuery = toml::from_str(content).unwrap();
        assert_eq!(query.name, "Recent Orders");
        assert_eq!(query.connection, "northwind");
        assert!(query.sql.is_some());
        assert!(query.params.is_some());
    }
}
