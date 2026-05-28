use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub type Variables = HashMap<String, String>;

#[derive(Debug, Serialize, Deserialize, Default)]
struct VarsFile {
    #[serde(default)]
    vars: HashMap<String, String>,
}

pub fn load_variables(project_dir: &Path) -> Result<Variables> {
    let path = project_dir.join(".based").join("vars.toml");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let file: VarsFile = toml::from_str(&content)?;
    Ok(file.vars)
}

pub fn save_variables(project_dir: &Path, vars: &Variables) -> Result<()> {
    let path = project_dir.join(".based").join("vars.toml");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = VarsFile { vars: vars.clone() };
    let content = toml::to_string_pretty(&file)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Replace `$VAR_NAME` occurrences. Unknown variables are left as-is.
pub fn substitute_dollar_vars(query: &str, vars: &Variables) -> String {
    let mut result = query.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${k}"), v);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_known_var() {
        let mut vars = HashMap::new();
        vars.insert("SCHEMA".into(), "public".into());
        let out = substitute_dollar_vars("SELECT * FROM $SCHEMA.users", &vars);
        assert_eq!(out, "SELECT * FROM public.users");
    }
}
