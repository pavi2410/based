// File-system watcher — watches `.based/` directory for config changes.
// Calls `on_change` whenever any file under `.based/` is modified.
// The caller (Project entity) schedules a config reload in response.

use std::path::PathBuf;

use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};

pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    /// Start watching `project_dir/.based/` (non-recursively).
    /// On any file-system event `on_change` is called.
    pub fn new(project_dir: PathBuf, on_change: impl Fn() + Send + 'static) -> Result<Self> {
        let watched_dir = project_dir.join(".based");

        let mut watcher = recommended_watcher(move |res: notify::Result<Event>| {
            if res.is_ok() {
                on_change();
            }
        })?;

        // Best-effort: if `.based/` doesn't exist yet the watcher is still
        // constructed; the watch call is skipped gracefully.
        if watched_dir.exists() {
            watcher.watch(&watched_dir, RecursiveMode::Recursive)?;
        }

        Ok(Self { _watcher: watcher })
    }
}
