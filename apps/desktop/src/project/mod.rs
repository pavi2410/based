// project/ — .based/ directory I/O: config.toml, saved queries, file watcher,
// variable resolution.

pub mod based_config;
pub mod config;
pub mod discovery;
pub mod queries;
pub mod reload;
pub mod variables;
pub mod watcher;

pub use based_config::load_workspace_seed;
pub use config::*;
pub use discovery::find_project_root;
pub use variables::*;

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{Context, EventEmitter, Global};

/// Loaded `$VAR` map from `.based/vars.toml`, available to query panels via [`gpui::App::global`].
#[derive(Default)]
pub struct ProjectVars {
    pub vars: HashMap<String, String>,
}

impl Global for ProjectVars {}

/// Keeps the `.based/` filesystem watcher alive for the process lifetime.
#[derive(Default)]
pub struct ConfigWatcherGlobal {
    _watcher: Option<watcher::ConfigWatcher>,
}

impl Global for ConfigWatcherGlobal {}

pub use reload::{ProjectRoot, RegistryRef, drain_pending_reload, install_reload_watcher};

pub enum ProjectEvent {
    ConfigReloaded,
}

pub struct Project {
    pub dir: PathBuf,
    pub config: ProjectConfig,
    _watcher: Option<watcher::ConfigWatcher>,
}

impl Project {
    pub fn open(dir: PathBuf, _cx: &mut Context<Self>) -> Self {
        let config = ProjectConfig::load(&dir).unwrap_or_default();
        let watcher = watcher::ConfigWatcher::new(dir.clone(), || {
            log::info!("based config changed — reload pending");
        })
        .inspect_err(|e| log::warn!("config watcher ({dir:?}): {e:#}"))
        .ok();
        Self {
            dir,
            config,
            _watcher: watcher,
        }
    }

    pub fn name(&self) -> &str {
        &self.config.project.name
    }
}

impl EventEmitter<ProjectEvent> for Project {}
