//! Reload `.based/` config, variables, queries, and connection list from disk.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

use gpui::{App, BorrowAppContext, Entity, Global};

use crate::connection::registry::ConnectionRegistry;
use crate::query_store::{QueryHistory, QueryStore, SavedQueries};

use super::{ProjectVars, load_variables, load_workspace_seed};

pub struct ConfigReloadSignal {
    tx: Sender<()>,
    pub rx: Receiver<()>,
}

impl ConfigReloadSignal {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }

    pub fn notify(&self) {
        let _ = self.tx.send(());
    }
}

impl Global for ConfigReloadSignal {}

pub struct ProjectRoot(pub std::path::PathBuf);

impl Global for ProjectRoot {}

pub struct RegistryRef(pub Entity<ConnectionRegistry>);

impl Global for RegistryRef {}

pub fn reload_from_disk(project_root: &Path, registry: &Entity<ConnectionRegistry>, cx: &mut App) {
    if let Ok(vars) = load_variables(project_root) {
        cx.update_global(|pv: &mut ProjectVars, _| pv.vars = vars);
    }

    let queries_dir = project_root.join(".based").join("local");
    let saved_path = project_root.join(".based").join("queries.toml");
    cx.update_global(|store: &mut QueryStore, _| {
        store.history = QueryHistory::load(&queries_dir);
        store.saved = SavedQueries::load(&saved_path);
    });

    let (_, entries) = load_workspace_seed(project_root);
    registry.update(cx, |reg, cx| {
        for entry in entries {
            if reg.get(&entry.id, cx).is_none() {
                reg.add(entry, cx);
            }
        }
    });

    log::info!("reloaded .based config for {}", project_root.display());
}

pub fn drain_pending_reload(cx: &mut App) {
    let mut needs_reload = false;
    if let Some(signal) = cx.try_global::<ConfigReloadSignal>() {
        while signal.rx.try_recv().is_ok() {
            needs_reload = true;
        }
    }
    if !needs_reload {
        return;
    }
    let root = cx.try_global::<ProjectRoot>().map(|p| p.0.clone());
    let registry = cx.try_global::<RegistryRef>().map(|r| r.0.clone());
    if let (Some(root), Some(registry)) = (root, registry) {
        reload_from_disk(&root, &registry, cx);
    }
}

pub fn install_reload_watcher(project_root: std::path::PathBuf, cx: &mut App) {
    let signal = ConfigReloadSignal::new();
    let notify_tx = signal.tx.clone();
    cx.set_global(signal);
    cx.set_global(ProjectRoot(project_root.clone()));

    let _ = super::watcher::ConfigWatcher::new(project_root, move || {
        let _ = notify_tx.send(());
    });
}
