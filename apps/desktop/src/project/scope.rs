//! The project one window has open: its root, manifest snapshot, `$VAR`s,
//! saved queries and history, and the watcher that notices edits under
//! `.based/`. Each workspace window owns one (see `Workspaces`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use super::context::ProjectContext;
use super::load_variables;
use super::settings::query_timeout_ms_to_secs;
use super::watcher::ConfigWatcher;
use crate::query_store::QueryStore;

pub struct ProjectState {
    /// `None`: no project is open in this window.
    root: Option<PathBuf>,
    context: Option<ProjectContext>,
    pub vars: HashMap<String, String>,
    pub queries: QueryStore,
    /// Fires when files under `.based/` change; drained on render.
    changes: Option<Receiver<()>>,
    _watcher: Option<ConfigWatcher>,
}

impl ProjectState {
    /// No project: queries and history live in memory only.
    pub fn none() -> Self {
        Self {
            root: None,
            context: None,
            vars: HashMap::new(),
            queries: QueryStore::new(None, None),
            changes: None,
            _watcher: None,
        }
    }

    /// Load the project at `root` and start watching it. A project whose manifest
    /// can't be read still opens, with no snapshot, so the window shows the error
    /// state rather than nothing.
    pub fn open(root: PathBuf) -> Self {
        let root = canonical_root(&root);
        let context = ProjectContext::load(root.clone())
            .map_err(|e| log::warn!("project load ({root:?}): {e:#}"))
            .ok();
        let vars = load_variables(&root).unwrap_or_else(|e| {
            log::warn!("vars.toml load ({root:?}): {e:#}");
            HashMap::new()
        });
        let queries = QueryStore::new(Some(root.clone()), context.as_ref().map(|c| &c.snapshot));
        let (tx, rx) = channel();
        let watcher = ConfigWatcher::new(root.clone(), move || {
            let _ = tx.send(());
        })
        .map_err(|e| log::warn!("config watcher install failed: {e:#}"))
        .ok();
        Self {
            root: Some(root),
            context,
            vars,
            queries,
            changes: Some(rx),
            _watcher: watcher,
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn is_open(&self) -> bool {
        self.root.is_some()
    }

    pub fn context(&self) -> Option<&ProjectContext> {
        self.context.as_ref()
    }

    /// `query_timeout` from `project.toml`, which overrides the user's
    /// preference in this window only.
    pub fn query_timeout_secs(&self) -> Option<u32> {
        let settings = self.context.as_ref()?.snapshot.manifest.settings.as_ref()?;
        settings.query_timeout.map(query_timeout_ms_to_secs)
    }

    pub fn set_context(&mut self, context: ProjectContext) {
        self.context = Some(context);
    }

    /// Whether `.based/` changed since the last call.
    pub fn take_changes(&self) -> bool {
        let Some(rx) = &self.changes else {
            return false;
        };
        let mut changed = false;
        while rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

/// One spelling per project folder (symlinks, `..` and trailing slashes
/// resolved), so the same project never gets two windows or two session slots.
pub fn canonical_root(root: &Path) -> PathBuf {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::none()
    }
}
