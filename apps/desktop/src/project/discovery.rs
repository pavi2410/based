use std::env;
use std::path::{Path, PathBuf};

use crate::app::paths::AppPaths;
use crate::app::prefs::NativePreferences;
use crate::project::personal::is_under_personal_root;

/// Resolve a picked path to a project root (directory containing `.based/`),
/// never inside the `personal` tree.
pub fn resolve_project_root(path: &Path, personal: &Path) -> Option<PathBuf> {
    let mut dir = path.to_path_buf();
    loop {
        if is_under_personal_root(&dir, personal) {
            return None;
        }
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
pub fn find_project_root(paths: &AppPaths) -> Option<PathBuf> {
    let personal = &paths.personal_root;
    if let Ok(dir) = env::var("BASED_PROJECT_DIR") {
        let p = PathBuf::from(dir);
        if !is_under_personal_root(&p, personal) && p.join(".based").is_dir() {
            return Some(p);
        }
    }
    if let Ok(cwd) = env::current_dir()
        && let Some(root) = resolve_project_root(&cwd, personal)
    {
        return Some(root);
    }
    NativePreferences::load_from(paths.prefs_path.clone())
        .last_opened_project
        .filter(|p| !is_under_personal_root(p, personal) && p.join(".based").is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_project_root_finds_normal_repo_based_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".based")).unwrap();
        let nested = dir.path().join("src");
        fs::create_dir_all(&nested).unwrap();
        let personal = PathBuf::from("/tmp/not-this-personal-root");
        assert_eq!(
            resolve_project_root(&nested, &personal).as_deref(),
            Some(dir.path())
        );
    }

    #[test]
    fn resolve_project_root_ignores_personal_tree_even_with_based_folder() {
        let dir = tempfile::tempdir().unwrap();
        let personal = dir.path().to_path_buf();
        fs::create_dir_all(personal.join(".based")).unwrap();
        fs::create_dir_all(personal.join("connections")).unwrap();
        assert_eq!(
            resolve_project_root(&personal.join("connections"), &personal),
            None
        );
        assert_eq!(resolve_project_root(&personal, &personal), None);
    }
}
