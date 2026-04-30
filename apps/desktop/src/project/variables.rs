// Variable resolution — $VAR_NAME substitution in query text.
// Variables are defined in .based/vars.toml under the [vars] table.
// Format:
// [vars]
// MY_VAR = "value"
// OTHER = "something"

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

/// Load variables from `.based/vars.toml`. Returns an empty map if the file
/// does not exist.
pub fn load_variables(project_dir: &Path) -> Result<Variables> {
    let path = project_dir.join(".based").join("vars.toml");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let file: VarsFile = toml::from_str(&content)?;
    Ok(file.vars)
}

/// Persist variables to `.based/vars.toml`.
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

/// Replace `$VAR_NAME` occurrences in `query` with values from `vars`.
/// Unknown variables are left as-is.
pub fn substitute(query: &str, vars: &Variables) -> String {
    let mut result = query.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${k}"), v);
    }
    result
}
