//! Expand `~` in SSH identity paths.

use std::path::PathBuf;

pub fn expand_tilde(path: &str) -> PathBuf {
    let path = path.trim();
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

pub fn expand_key_path(path: &str) -> PathBuf {
    expand_tilde(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_joins_home() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(expand_tilde("~/id_ed25519"), home.join("id_ed25519"));
        assert_eq!(expand_tilde("~"), home);
    }

    #[test]
    fn expand_tilde_leaves_absolute_paths() {
        assert_eq!(
            expand_tilde("/abs/id_ed25519"),
            PathBuf::from("/abs/id_ed25519")
        );
    }
}
