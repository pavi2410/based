use std::path::PathBuf;

/// Project root: directory containing `.based/`.
///
/// `BASED_PROJECT_DIR` wins if set and points at a directory with `.based/`.
pub fn find_project_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BASED_PROJECT_DIR") {
        let p = PathBuf::from(dir);
        if p.join(".based").is_dir() {
            return Some(p);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".based").is_dir() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}
