use std::env;
use std::path::{Path, PathBuf};

use crate::app::prefs::NativePreferences;
use crate::project::personal::{is_personal_tree, is_under_personal_root, personal_root};

/// Resolve a picked path to a project root (directory containing `.based/`).
pub fn resolve_project_root(path: &Path) -> Option<PathBuf> {
    resolve_project_root_excluding(path, &personal_root())
}

fn resolve_project_root_excluding(path: &Path, personal: &Path) -> Option<PathBuf> {
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
pub fn find_project_root() -> Option<PathBuf> {
    if let Ok(dir) = env::var("BASED_PROJECT_DIR") {
        let p = PathBuf::from(dir);
        if !is_personal_tree(&p) && p.join(".based").is_dir() {
            return Some(p);
        }
    }
    if let Ok(cwd) = env::current_dir()
        && let Some(root) = resolve_project_root(&cwd)
    {
        return Some(root);
    }
    NativePreferences::load()
        .last_opened_project
        .filter(|p| !is_personal_tree(p) && p.join(".based").is_dir())
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
            resolve_project_root_excluding(&nested, &personal).as_deref(),
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
            resolve_project_root_excluding(&personal.join("connections"), &personal),
            None
        );
        assert_eq!(resolve_project_root_excluding(&personal, &personal), None);
    }
}
