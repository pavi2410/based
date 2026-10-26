//! Shared TOML file I/O for small per-user state files (prefs, updater state).

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// File contents, empty when missing or unreadable.
pub fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Parse `raw`, falling back to `T::default()` when empty or invalid.
pub fn parse<T: DeserializeOwned + Default>(raw: &str, path: &Path) -> T {
    if raw.is_empty() {
        return T::default();
    }
    toml::from_str(raw).unwrap_or_else(|e| {
        log::debug!("toml file: using defaults ({path:?}: {e})");
        T::default()
    })
}

/// Write `value`, creating parent directories.
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let encoded = toml::to_string_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    fs::write(path, encoded).map_err(|e| format!("write {path:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Default, Debug, PartialEq, Serialize, Deserialize)]
    struct Doc {
        #[serde(default)]
        n: u32,
    }

    #[test]
    fn round_trips_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("doc.toml");
        assert_eq!(parse::<Doc>(&read(&path), &path), Doc::default());
        write(&path, &Doc { n: 7 }).unwrap();
        assert_eq!(parse::<Doc>(&read(&path), &path), Doc { n: 7 });
        assert_eq!(parse::<Doc>("not = [toml", &path), Doc::default());
    }
}
