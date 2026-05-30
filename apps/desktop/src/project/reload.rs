//! Reload `.based/` project snapshot, variables, queries, and connections from disk.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

use gpui::{App, BorrowAppContext, Entity, Global};

use crate::connection::registry::ConnectionRegistry;
use crate::project::context::ProjectContext;
use crate::project::loader::entry_from_project;
use crate::project::settings::apply_project_settings;
use crate::query_store::QueryStore;

use super::{ProjectVars, load_variables};

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

    let ctx = match ProjectContext::load(project_root.to_path_buf()) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("project reload failed: {e:#}");
            return;
        }
    };

    let queries_dir = project_root.join(".based").join("local");
    cx.update_global(|store: &mut QueryStore, _| {
        store.history = crate::query_store::history::QueryHistory::load(&queries_dir);
        store.apply_snapshot(&ctx.snapshot);
    });

    let mut entries = Vec::new();
    for conn in &ctx.snapshot.connections {
        match entry_from_project(conn) {
            Ok(e) => entries.push(e),
            Err(e) => log::warn!("connection {} skipped: {e:#}", conn.id),
        }
    }

    registry.update(cx, |reg, cx| {
        reg.sync_project_entries(entries, cx);
    });

    cx.set_global(ctx.clone());
    apply_project_settings(&ctx.snapshot.manifest, cx);

    log::info!("reloaded .based project for {}", project_root.display());
}

/// Returns `true` when a reload was applied (caller should refresh workspace-local state).
pub fn drain_pending_reload(cx: &mut App) -> bool {
    let mut needs_reload = false;
    if let Some(signal) = cx.try_global::<ConfigReloadSignal>() {
        while signal.rx.try_recv().is_ok() {
            needs_reload = true;
        }
    }
    if !needs_reload {
        return false;
    }
    let root = cx.try_global::<ProjectRoot>().map(|p| p.0.clone());
    let registry = cx.try_global::<RegistryRef>().map(|r| r.0.clone());
    if let (Some(root), Some(registry)) = (root, registry) {
        reload_from_disk(&root, &registry, cx);
        return true;
    }
    false
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
