use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VariableError {
    #[error("Variable not found: {0}")]
    VariableNotFound(String),
    #[error("Failed to read .env file: {0}")]
    EnvFileReadError(String),
    #[error("Failed to parse .env file: {0}")]
    EnvFileParseError(String),
}

/// Parse .env file and return HashMap of variables
pub fn parse_env_file(content: &str) -> Result<HashMap<String, String>, VariableError> {
    let mut vars = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // Parse KEY=VALUE
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim();

            // Remove quotes if present
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            };

            vars.insert(key, value);
        }
    }

    Ok(vars)
}

/// Load .env file from project directory
pub fn load_env_file(project_path: &str) -> Result<HashMap<String, String>, VariableError> {
    let env_path = Path::new(project_path).join(".based/.env");

    if !env_path.exists() {
        // Return empty map if .env doesn't exist
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&env_path)
        .map_err(|e| VariableError::EnvFileReadError(e.to_string()))?;

    parse_env_file(&content)
}

/// Resolve variables in a string (format: ${env:VAR_NAME})
pub fn resolve_variables(
    value: &str,
    env_vars: &HashMap<String, String>,
) -> Result<String, VariableError> {
    let var_regex = Regex::new(r"\$\{env:([^}]+)\}").unwrap();
    let mut result = value.to_string();

    for cap in var_regex.captures_iter(value) {
        let var_name = &cap[1];
        let full_match = &cap[0];

        // Try to get from env_vars HashMap first, then from system env
        let replacement = if let Some(value) = env_vars.get(var_name) {
            value.clone()
        } else if let Ok(value) = std::env::var(var_name) {
            value
        } else {
            return Err(VariableError::VariableNotFound(var_name.to_string()));
        };

        result = result.replace(full_match, &replacement);
    }

    Ok(result)
}

/// Resolve all variables in a ConnectionConfig
pub fn resolve_connection_config(
    config: &crate::project_types::ConnectionConfig,
    env_vars: &HashMap<String, String>,
) -> Result<crate::project_types::ConnectionConfig, VariableError> {
    Ok(crate::project_types::ConnectionConfig {
        path: config
            .path
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        url: config
            .url
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        host: config
            .host
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        port: config.port,
        database: config
            .database
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        username: config
            .username
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        password: config
            .password
            .as_ref()
            .map(|v| resolve_variables(v, env_vars))
            .transpose()?,
        sslmode: config.sslmode.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_file() {
        let content = r#"
# Comment
DB_HOST=localhost
DB_PORT=5432
DB_USER="admin"
DB_PASS='secret'

# Another comment
API_KEY=test123
"#;

        let vars = parse_env_file(content).unwrap();
        assert_eq!(vars.get("DB_HOST"), Some(&"localhost".to_string()));
        assert_eq!(vars.get("DB_PORT"), Some(&"5432".to_string()));
        assert_eq!(vars.get("DB_USER"), Some(&"admin".to_string()));
        assert_eq!(vars.get("DB_PASS"), Some(&"secret".to_string()));
        assert_eq!(vars.get("API_KEY"), Some(&"test123".to_string()));
    }

    #[test]
    fn test_resolve_variables() {
        let mut env_vars = HashMap::new();
        env_vars.insert("DB_HOST".to_string(), "localhost".to_string());
        env_vars.insert("DB_PORT".to_string(), "5432".to_string());

        let input = "postgres://${env:DB_HOST}:${env:DB_PORT}/mydb";
        let result = resolve_variables(input, &env_vars).unwrap();
        assert_eq!(result, "postgres://localhost:5432/mydb");
    }

    #[test]
    fn test_resolve_variables_not_found() {
        let env_vars = HashMap::new();
        let input = "${env:MISSING_VAR}";
        let result = resolve_variables(input, &env_vars);
        assert!(result.is_err());
    }
}
