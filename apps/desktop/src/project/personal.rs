//! Personal based-dir (`~/.config/based`) — user-local connections, not a project.

use std::env;
use std::path::{Path, PathBuf};

/// Resolve the personal based-dir.
///
/// `XDG_CONFIG_HOME/based` when that env var is set and non-empty, otherwise
/// `dirs::home_dir()/.config/based`. Do not use `dirs::config_dir()` (macOS
/// Application Support / Windows AppData). Do not use `~/.based`.
pub fn personal_root() -> PathBuf {
    personal_root_from(
        env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        dirs::home_dir(),
    )
}

pub fn personal_root_from(xdg_config_home: Option<String>, home: Option<PathBuf>) -> PathBuf {
    match xdg_config_home {
        Some(xdg) => PathBuf::from(xdg).join("based"),
        None => home.unwrap_or_default().join(".config").join("based"),
    }
}

/// True when `path` is the personal based-dir or a file inside it.
pub fn is_personal_tree(path: &Path) -> bool {
    is_under_personal_root(path, &personal_root())
}

pub fn is_under_personal_root(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn personal_root_uses_xdg_config_home_when_set() {
        let root = personal_root_from(
            Some("/xdg-config".into()),
            Some(PathBuf::from("/home/pavi")),
        );
        assert_eq!(root, PathBuf::from("/xdg-config/based"));
    }

    #[test]
    fn personal_root_falls_back_to_home_config_based() {
        let root = personal_root_from(None, Some(PathBuf::from("/Users/pavi")));
        assert_eq!(root, PathBuf::from("/Users/pavi/.config/based"));
    }

    #[test]
    fn is_under_personal_root_for_root_and_child() {
        let root = PathBuf::from("/Users/pavi/.config/based");
        assert!(is_under_personal_root(&root, &root));
        assert!(is_under_personal_root(
            &root.join("connections/pg.toml"),
            &root
        ));
        assert!(!is_under_personal_root(
            Path::new("/Users/pavi/Projects/app"),
            &root
        ));
    }
}
