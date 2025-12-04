use crate::project_types::*;
use crate::variables::{load_env_file, VariableError};
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

/// Load environment variables from .env file
#[command]
pub async fn load_env_file_command(project_path: String) -> Result<HashMap<String, String>, String> {
    load_env_file(&project_path)
        .map_err(|e| format!("Failed to load .env file: {}", e))
}

/// Resolve connection config with variable interpolation
#[command]
pub async fn resolve_connection_config_command(
    project_path: String,
    conn_key: String,
) -> Result<ConnectionConfig, String> {
    // Read project config
    let config = read_project_config(project_path.clone()).await?;

    // Get connection config
    let conn_config = config
        .connection
        .get(&conn_key)
        .ok_or_else(|| format!("Connection '{}' not found in config", conn_key))?;

    // Connection config is already complete, just return it
    // Secret resolution happens at connection time in project_db_commands
    Ok(conn_config.clone())
}

/// List all query files in the project
#[command]
pub async fn list_query_files(project_path: String) -> Result<Vec<String>, String> {
    let queries_path = Path::new(&project_path).join(".based/queries");

    if !queries_path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

    for entry in WalkDir::new(&queries_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());

            // Only include .sqlx and .mongox files
            if ext == Some("sqlx") || ext == Some("mongox") {
                if let Ok(relative_path) = path.strip_prefix(&queries_path) {
                    if let Some(path_str) = relative_path.to_str() {
                        files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Read a query file and parse its metadata
#[command]
pub async fn read_query_file(
    project_path: String,
    query_path: String,
) -> Result<QueryFile, String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&query_path);

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read query file: {}", e))?;

    // Parse YAML frontmatter
    let (metadata, query_content) = parse_query_frontmatter(&content)
        .map_err(|e| format!("Failed to parse query frontmatter: {}", e))?;

    Ok(QueryFile {
        path: query_path.clone(),
        name: metadata.name.clone(),
        connection: metadata.connection.clone(),
        content: query_content,
        metadata,
    })
}

/// Write a query file with metadata
#[command]
pub async fn write_query_file(
    project_path: String,
    query_path: String,
    metadata: QueryMetadata,
    content: String,
) -> Result<(), String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&query_path);

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Serialize metadata to YAML
    let yaml_metadata = serde_yaml::to_string(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

    // Combine frontmatter and content
    let full_content = format!("---\n{}---\n\n{}", yaml_metadata, content);

    fs::write(&file_path, full_content)
        .map_err(|e| format!("Failed to write query file: {}", e))?;

    Ok(())
}

/// Delete a query file
#[command]
pub async fn delete_query_file(
    project_path: String,
    query_path: String,
) -> Result<(), String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&query_path);

    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete query file: {}", e))?;

    Ok(())
}

/// Parse YAML frontmatter from query file
fn parse_query_frontmatter(content: &str) -> Result<(QueryMetadata, String), ProjectError> {
    // Check if content starts with ---
    if !content.starts_with("---") {
        return Err(ProjectError::InvalidFrontmatter(
            "Query file must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the end of frontmatter
    let lines: Vec<&str> = content.lines().collect();
    let mut end_index = 0;

    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end_index = i;
            break;
        }
    }

    if end_index == 0 {
        return Err(ProjectError::InvalidFrontmatter(
            "Could not find end of YAML frontmatter (---)".to_string(),
        ));
    }

    // Extract frontmatter (excluding the --- delimiters)
    let frontmatter = lines[1..end_index].join("\n");

    // Extract query content (everything after the second ---)
    let query_content = lines[end_index + 1..].join("\n").trim().to_string();

    // Parse YAML frontmatter
    let metadata: QueryMetadata = serde_yaml::from_str(&frontmatter)?;

    Ok((metadata, query_content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_frontmatter() {
        let content = r#"---
name: Test Query
database: main
description: A test query
tags: [test, example]
favorite: false
---

SELECT * FROM users;
"#;

        let (metadata, query) = parse_query_frontmatter(content).unwrap();
        assert_eq!(metadata.name, "Test Query");
        assert_eq!(metadata.database, "main");
        assert_eq!(query, "SELECT * FROM users;");
    }
}
