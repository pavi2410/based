use std::path::{Path, PathBuf};

pub fn walk_files(root: &Path, suffix: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_files_inner(root, suffix, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_files_inner(dir: &Path, suffix: &str, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_files_inner(&path, suffix, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(suffix))
        {
            out.push(path);
        }
    }
    Ok(())
}
