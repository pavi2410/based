use std::path::{Path, PathBuf};

use crate::app::prefs::NativePreferences;

/// Resolve a picked path to a project root (directory containing `.based/`).
pub fn resolve_project_root(path: &Path) -> Option<PathBuf> {
    let mut dir = path.to_path_buf();
    loop {
        if dir.join(".based").is_dir() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Project root: directory containing `.based/`.
///
/// Resolution order:
/// 1. `BASED_PROJECT_DIR` when set and valid
/// 2. Ancestor walk from process cwd (terminal launch)
/// 3. Last opened project from native preferences (Dock / GUI launch)
pub fn find_project_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BASED_PROJECT_DIR") {
        let p = PathBuf::from(dir);
        if p.join(".based").is_dir() {
            return Some(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Some(root) = resolve_project_root(&cwd)
    {
        return Some(root);
    }
    NativePreferences::load()
        .last_opened_project
        .filter(|p| p.join(".based").is_dir())
}
