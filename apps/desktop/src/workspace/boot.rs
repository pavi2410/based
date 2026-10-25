//! Process-level inputs [`Workspace`](super::Workspace) reads at startup.

use std::path::PathBuf;

use crate::project::{find_project_root, personal::personal_root};

/// Where the workspace finds its project and personal `.based/` trees.
///
/// Production uses [`WorkspaceEnv::from_process`]; tests pass explicit paths
/// to [`Workspace::with_env`](super::Workspace::with_env).
#[derive(Clone, Debug)]
pub struct WorkspaceEnv {
    /// Folder containing `.based/`, if a project is open.
    pub project_dir: Option<PathBuf>,
    /// Per-user config tree (`~/.config/based`).
    pub personal_root: PathBuf,
}

impl WorkspaceEnv {
    pub fn from_process() -> Self {
        Self {
            project_dir: find_project_root(),
            personal_root: personal_root(),
        }
    }
}
