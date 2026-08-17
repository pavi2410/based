use std::fs;
use std::path::{Path, PathBuf};

/// Recursively collect `*.toml` files, skipping names that start with `_`.
pub fn walk_toml_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_toml_files_inner(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_toml_files_inner(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_toml_files_inner(&path, out)?;
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        if name.ends_with(".toml") {
            out.push(path);
        }
    }
    Ok(())
}

/// Path relative to `connections/` or `queries/`, without the TOML suffix.
///
/// Accepts plain `*.toml` and the legacy `*.conn.toml` / `*.query.toml` midfixes.
pub fn rel_id(rel: &Path) -> String {
    let s = rel.to_string_lossy().replace('\\', "/");
    strip_based_toml_suffix(&s).to_string()
}

pub fn strip_based_toml_suffix(path: &str) -> &str {
    path.strip_suffix(".conn.toml")
        .or_else(|| path.strip_suffix(".query.toml"))
        .or_else(|| path.strip_suffix(".toml"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "").unwrap();
    }

    #[test]
    fn walk_toml_files_collects_nested_toml_and_skips_underscore_and_non_toml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "northwind.toml");
        write(root, "local/postgres.toml");
        write(root, "_template.toml");
        write(root, "local/_draft.toml");
        write(root, "notes.md");
        write(root, "sidecar.sql");

        let files = walk_toml_files(root).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "local/postgres.toml".to_string(),
                "northwind.toml".to_string()
            ]
        );
    }

    #[test]
    fn rel_id_strips_toml_and_legacy_midfixes() {
        assert_eq!(rel_id(Path::new("local/northwind.toml")), "local/northwind");
        assert_eq!(
            rel_id(Path::new("local/northwind.conn.toml")),
            "local/northwind"
        );
        assert_eq!(
            rel_id(Path::new("local/northwind/recent-orders.query.toml")),
            "local/northwind/recent-orders"
        );
        assert_eq!(rel_id(Path::new("pg-list-tables.toml")), "pg-list-tables");
    }
}
