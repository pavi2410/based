// project/ — .based/ directory I/O: config.toml, saved queries, file watcher,
// variable resolution.

pub mod config;
pub mod queries;
pub mod variables;
pub mod watcher;

pub use config::*;
pub use queries::*;
pub use variables::*;

use std::path::PathBuf;

use gpui::{Context, EventEmitter};

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
        // Watcher integration deferred to Phase 2b.
        Self {
            dir,
            config,
            _watcher: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.config.project.name
    }
}

impl EventEmitter<ProjectEvent> for Project {}
