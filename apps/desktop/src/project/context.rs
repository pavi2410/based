use std::path::PathBuf;
use std::process::Command;

use based_project::ProjectSnapshot;
use gpui::Global;

/// Runtime project context for title bar and reload.
#[derive(Clone)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub snapshot: ProjectSnapshot,
    pub git_branch: Option<String>,
}

impl ProjectContext {
    pub fn load(root: PathBuf) -> anyhow::Result<Self> {
        let snapshot = based_project::load_project(&root)?;
        let git_branch = read_git_branch(&root);
        Ok(Self {
            root,
            snapshot,
            git_branch,
        })
    }

    pub fn project_name(&self) -> &str {
        &self.snapshot.manifest.name
    }

    pub fn active_env(&self) -> &str {
        &self.snapshot.active_environment
    }
}

impl Global for ProjectContext {}

pub fn read_git_branch(root: &PathBuf) -> Option<String> {
    Command::new("git")
        .args(["-C", root.to_str()?, "branch", "--show-current"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
