//! Parse and upsert `KEY=VALUE` lines in a based-dir `.env` file.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Build `BASED_<ID>_PASSWORD` / `BASED_<ID>_URL` from a connection relative id.
pub fn secret_env_key(relative_id: &str, suffix: &str) -> String {
    let body = relative_id.replace(['/', '-'], "_").to_ascii_uppercase();
    format!("BASED_{body}_{suffix}")
}

pub fn load_env_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse_env(&raw))
}

pub fn upsert_env_file(path: &Path, key: &str, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let existing = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
    } else {
        String::new()
    };
    let updated = upsert_env(&existing, key, value);
    fs::write(path, updated).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn parse_env(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.insert(key.to_string(), unquote(value.trim()));
    }
    out
}

fn upsert_env(raw: &str, key: &str, value: &str) -> String {
    let encoded = format!("{key}={}", quote_env_value(value));
    let mut replaced = false;
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
        if body.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            lines.push(encoded.clone());
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        if !lines.is_empty() && !lines.last().is_some_and(|l| l.is_empty()) {
            // keep a trailing newline by joining with \n below
        }
        lines.push(encoded);
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
        }
    }
    value.to_string()
}

fn quote_env_value(value: &str) -> String {
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '#' | '"' | '\'' | '='))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_env_key_uppercases_slug() {
        assert_eq!(
            secret_env_key("local/northwind", "PASSWORD"),
            "BASED_LOCAL_NORTHWIND_PASSWORD"
        );
        assert_eq!(secret_env_key("analytics", "URL"), "BASED_ANALYTICS_URL");
    }

    #[test]
    fn load_missing_env_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let map = load_env_file(&dir.path().join(".env")).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn upsert_creates_and_replaces_without_dropping_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "# keep me\nOTHER=1\nBASED_OLD_PASSWORD=old\n").unwrap();
        upsert_env_file(&path, "BASED_OLD_PASSWORD", "new secret").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("# keep me"));
        assert!(raw.contains("OTHER=1"));
        assert!(raw.contains("BASED_OLD_PASSWORD="));
        assert!(!raw.contains("=old"));
        let map = load_env_file(&path).unwrap();
        assert_eq!(map.get("OTHER").map(String::as_str), Some("1"));
        assert_eq!(
            map.get("BASED_OLD_PASSWORD").map(String::as_str),
            Some("new secret")
        );
    }
}
