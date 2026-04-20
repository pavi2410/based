use crate::project_types::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tauri::command;
use walkdir::WalkDir;

// Seed SQL for the sample project. Kept as a single string so we can
// ship one sqlx execute_many call. Small enough to be human-readable
// but meaty enough that a new user can try joins, aggregations, and
// filters without writing any inserts of their own.
const SAMPLE_SEED_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS posts (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    published INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY,
    post_id INTEGER NOT NULL REFERENCES posts(id),
    author TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO users (id, name, email) VALUES
    (1, 'Ada Lovelace', 'ada@example.com'),
    (2, 'Alan Turing', 'alan@example.com'),
    (3, 'Grace Hopper', 'grace@example.com');
INSERT INTO posts (id, user_id, title, body, published) VALUES
    (1, 1, 'Notes on the Analytical Engine', 'Sketches of what could be.', 1),
    (2, 2, 'On Computable Numbers', 'A machine that computes anything computable.', 1),
    (3, 3, 'COBOL is coming', 'English-like syntax for business computing.', 0);
INSERT INTO comments (id, post_id, author, body) VALUES
    (1, 1, 'Charles', 'Brilliant!'),
    (2, 1, 'Mary', 'I agree.'),
    (3, 2, 'Max', 'Decidable problems only?');
"#;

// Structured error types for project operations live in `crate::error`
// (see `ProjectError` there). The commands below still return
// `Result<T, String>` for IPC compatibility, but callers inside the
// backend should prefer `crate::error::ProjectError` / `AppError`.

/// Initialize a new Based project in the given directory
#[command]
#[specta::specta]
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

/// Create a turnkey sample project at `{parent_dir}/{name}`.
///
/// Spins up a SQLite database seeded with a tiny users/posts/comments
/// schema so new users can immediately browse, filter, run joins, and
/// try EXPLAIN on realistic-shaped data without first standing up
/// their own database.
///
/// Returns the absolute path of the newly-created project so the
/// frontend can open it straight away.
#[command]
#[specta::specta]
pub async fn create_sample_project(parent_dir: String, name: String) -> Result<String, String> {
    let project_path = Path::new(&parent_dir).join(&name);
    if project_path.exists() && project_path.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!(
            "Destination {} is not empty; pick a different name.",
            project_path.display()
        ));
    }
    fs::create_dir_all(&project_path)
        .map_err(|e| format!("Failed to create project dir: {}", e))?;

    let project_path_str = project_path
        .to_str()
        .ok_or_else(|| "Project path is not valid UTF-8".to_string())?
        .to_string();

    // Delegate the scaffolding (dirs + gitignore + default config) to
    // the existing initializer so there's one source of truth for
    // project layout.
    initialize_project(project_path_str.clone()).await?;

    // Seed the SQLite sample database.
    let db_path = project_path.join("sample.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = sqlx::sqlite::SqlitePool::connect(&db_url)
        .await
        .map_err(|e| format!("Failed to create sample.db: {}", e))?;
    {
        use sqlx::Executor;
        pool.execute(SAMPLE_SEED_SQL)
            .await
            .map_err(|e| format!("Failed to seed sample data: {}", e))?;
    }
    pool.close().await;

    // Rewrite config.toml with a connection pointing to the new DB.
    let mut config = read_project_config(project_path_str.clone()).await?;
    config.name = name.clone();
    config.description = Some("Sample project with a tiny blog-style SQLite dataset.".to_string());
    config.connection.insert(
        "sample".to_string(),
        ConnectionConfig {
            label: Some("Sample DB".to_string()),
            engine: Engine::Sqlite,
            group: Some("local".to_string()),
            disabled: None,
            order: Some(1),
            color: None,
            icon: None,
            file: Some("sample.db".to_string()),
            readonly: None,
            url: None,
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            ssl: None,
        },
    );
    write_project_config(project_path_str.clone(), config).await?;

    Ok(project_path_str)
}

/// Read and parse project config
#[command]
#[specta::specta]
pub async fn read_project_config(project_path: String) -> Result<ProjectConfig, String> {
    let config_path = Path::new(&project_path).join(".based/config.toml");

    if !config_path.exists() {
        return Err(format!("Project not initialized at: {}", project_path));
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;

    let config: ProjectConfig =
        toml::from_str(&content).map_err(|e| format!("Failed to parse config.toml: {}", e))?;

    Ok(config)
}

/// Write project config
#[command]
#[specta::specta]
pub async fn write_project_config(
    project_path: String,
    config: ProjectConfig,
) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".based/config.toml");

    let content = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, content).map_err(|e| format!("Failed to write config.toml: {}", e))?;

    Ok(())
}

/// List all saved queries in the project with summary info
#[command]
#[specta::specta]
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
#[specta::specta]
pub async fn get_saved_query(project_path: String, filename: String) -> Result<SavedQuery, String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&filename);

    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read query file: {}", e))?;

    let mut query: SavedQuery =
        toml::from_str(&content).map_err(|e| format!("Failed to parse query file: {}", e))?;

    // Set filename (not in TOML)
    query.filename = filename;

    Ok(query)
}

/// Save a query file (create or update)
#[command]
#[specta::specta]
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
        return Err(format!(
            "Cannot write to directory: {}",
            file_path.display()
        ));
    }

    // Serialize to TOML
    let content =
        toml::to_string_pretty(&query).map_err(|e| format!("Failed to serialize query: {}", e))?;

    fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write query file '{}': {}", filename, e))?;

    Ok(())
}

/// Delete a saved query file
#[command]
#[specta::specta]
pub async fn delete_saved_query(project_path: String, filename: String) -> Result<(), String> {
    let file_path = Path::new(&project_path)
        .join(".based/queries")
        .join(&filename);

    fs::remove_file(&file_path).map_err(|e| format!("Failed to delete query file: {}", e))?;

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

    #[tokio::test]
    async fn test_create_sample_project_scaffolds_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let parent = tmp.path().to_str().unwrap().to_string();
        let name = "sample-check".to_string();

        let created = create_sample_project(parent.clone(), name.clone())
            .await
            .expect("create_sample_project");
        let created_path = Path::new(&created);

        assert!(created_path.join(".based").join("config.toml").exists());
        assert!(created_path.join("sample.db").exists());

        let cfg = read_project_config(created.clone()).await.expect("read cfg");
        assert_eq!(cfg.name, name);
        let conn = cfg.connection.get("sample").expect("sample connection");
        assert!(matches!(conn.engine, Engine::Sqlite));
        assert_eq!(conn.file.as_deref(), Some("sample.db"));
    }

    #[tokio::test]
    async fn test_create_sample_project_refuses_nonempty_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let parent = tmp.path().to_str().unwrap().to_string();
        let name = "dup".to_string();

        // Seed a file in the destination so it's non-empty.
        let dest = Path::new(&parent).join(&name);
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("marker"), "hi").unwrap();

        let err = create_sample_project(parent, name)
            .await
            .expect_err("expected failure on non-empty dir");
        assert!(err.contains("not empty"));
    }
}
