// project/ — `.based/` directory I/O: project load, file watcher, variable resolution.

pub mod context;
pub mod discovery;
pub mod loader;
pub mod open;
pub mod pick;
pub mod reload;
pub mod settings;
pub mod variables;
pub mod watcher;

pub use context::ProjectContext;
pub use discovery::find_project_root;
pub use open::{prompt_open_project_in_new_window, prompt_open_project_in_window};
pub use variables::*;

use std::collections::HashMap;

use gpui::Global;

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
