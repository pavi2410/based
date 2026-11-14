//! Reload one window's `.based/` project from disk: snapshot, variables,
//! queries and connections.

use std::fs;

use gpui_kit::{App, WindowId};

use crate::connection::ConnectionOrigin;
use crate::project::context::ProjectContext;
use crate::project::loader::load_entries_from_based_dir;
use crate::project::settings::apply_project_settings;
use crate::query_store::QueryHistory;
use crate::workspace::Workspaces;
use crate::workspace::notify as toast;

use super::load_variables;

/// Re-read the project open in `window` and update its connections.
pub fn reload_from_disk(window: WindowId, cx: &mut App) {
    let Some(root) = Workspaces::project(window, cx).and_then(|p| p.root().map(ToOwned::to_owned))
    else {
        return;
    };
    let ctx = match ProjectContext::load(root.clone()) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("project reload failed: {e:#}");
            return;
        }
    };
    let vars = load_variables(&root).ok();
    let queries_dir = root.join(".based").join("local");
    let _ = fs::create_dir_all(&queries_dir);
    Workspaces::update_project(window, cx, |project| {
        if let Some(vars) = vars {
            project.vars = vars;
        }
        project.queries.history_dir = Some(queries_dir.clone());
        project.queries.history = QueryHistory::load(&queries_dir);
        project.queries.apply_snapshot(&ctx.snapshot);
        project.set_context(ctx.clone());
    });

    let based_dir = root.join(".based");
    let loaded = load_entries_from_based_dir(&based_dir, ConnectionOrigin::Project);
    if let Some(registry) = Workspaces::registry_for_window(window, cx) {
        registry.update(cx, |reg, cx| {
            reg.sync_origin_entries(ConnectionOrigin::Project, loaded.entries, cx);
        });
    }
    if let Some(note) = toast::skipped_connections_notice(cx, &based_dir, &loaded.skipped)
        && toast::push_notification(cx, note)
    {
        toast::remember_skipped_notice(cx, &based_dir, &loaded.skipped);
    }
    let skipped_queries = &ctx.snapshot.skipped_queries;
    if let Some(note) = toast::skipped_queries_notice(cx, &based_dir, skipped_queries)
        && toast::push_notification(cx, note)
    {
        toast::remember_skipped_queries(cx, &based_dir, skipped_queries);
    }
    apply_project_settings(&ctx.snapshot.manifest, cx);

    log::info!("reloaded .based project for {}", root.display());
}

/// Reload `window`'s project when its files changed. Returns `true` when it did.
pub fn drain_pending_reload(window: WindowId, cx: &mut App) -> bool {
    let changed = Workspaces::project(window, cx).is_some_and(|p| p.take_changes());
    if changed {
        reload_from_disk(window, cx);
    }
    changed
}
