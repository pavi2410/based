//! Open a `.based/` project from the GUI — in-place switch or new process.

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use gpui::{App, BorrowAppContext, Entity, SharedString, Window, prelude::*};
use gpui_component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
};

use crate::app::prefs;
use crate::app::quit;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{live_connection_count, live_project_connection_count};
use crate::project::ProjectContext;
use crate::project::ProjectVars;
use crate::project::discovery::resolve_project_root;
use crate::project::reload::{
    ProjectRoot, RegistryRef, install_reload_watcher, reload_from_disk, stop_reload_watcher,
};
use crate::query_store::QueryStore;
use crate::workspace::Workspace;
use crate::workspace::WorkspaceRef;
use crate::workspace::notify;

use super::pick;
use super::settings::apply_project_settings;

/// After folder pick: open in the current window (may confirm when connections/tabs are dirty).
pub fn prompt_open_project_in_window(cx: &mut App) {
    spawn_pick_and_then(cx, ProjectOpenMode::InWindow);
}

/// After folder pick: spawn a new Based process with `BASED_PROJECT_DIR`.
pub fn prompt_open_project_in_new_window(cx: &mut App) {
    spawn_pick_and_then(cx, ProjectOpenMode::NewProcess);
}

/// Unload the current `.based/` project in this window.
pub fn request_close_project_in_window(cx: &mut App) {
    if !has_open_project(cx) {
        return;
    }
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
        return;
    };
    // Always queue onto the next render. Popup-menu / palette clicks often have
    // no `active_window()`, so completing here would silently no-op when there
    // are no live connections (the path that skips the confirm dialog).
    ws.update(cx, |workspace, cx| {
        workspace.pending_project_close_confirm = true;
        cx.notify();
    });
}

pub fn has_open_project(cx: &App) -> bool {
    cx.try_global::<ProjectRoot>().is_some()
}

#[derive(Clone, Copy)]
enum ProjectOpenMode {
    InWindow,
    NewProcess,
}

fn spawn_pick_and_then(cx: &mut App, mode: ProjectOpenMode) {
    cx.spawn(async move |cx| {
        let picked = pick::pick_project_folder(cx).await;
        cx.update(|cx| {
            let Some(picked) = picked else {
                return;
            };
            let Some(root) = resolve_project_root(&picked) else {
                notify::push_error(
                    cx,
                    "Open Project",
                    "Selected folder is not a Based project (no .based/ directory found).",
                );
                return;
            };
            match mode {
                ProjectOpenMode::InWindow => request_open_project_in_window(root, cx),
                ProjectOpenMode::NewProcess => open_project_in_new_process(root, cx),
            }
        });
    })
    .detach();
}

/// Validate and queue or complete an in-place project switch.
pub fn request_open_project_in_window(root: PathBuf, cx: &mut App) {
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
        return;
    };
    if is_same_project(&root, cx) {
        return;
    }
    if switch_needs_confirm(&ws, cx) {
        ws.update(cx, |workspace, cx| {
            workspace.pending_project_switch = Some(root);
            workspace.pending_project_switch_confirm = true;
            cx.notify();
        });
        return;
    }
    let Some(handle) = cx.active_window() else {
        return;
    };
    let _ = handle.update(cx, |_, window, cx| {
        complete_project_switch_in_window(root, window, cx);
    });
}

pub fn open_project_in_new_process(root: PathBuf, cx: &mut App) {
    prefs::record_opened_project(root.clone(), cx);
    let exe = match env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            notify::push_error(
                cx,
                "Open Project",
                format!("Could not locate app binary: {e:#}"),
            );
            return;
        }
    };
    match process::Command::new(exe)
        .env("BASED_PROJECT_DIR", &root)
        .spawn()
    {
        Ok(_) => {}
        Err(e) => {
            notify::push_error(
                cx,
                "Open Project",
                format!("Could not open new window: {e:#}"),
            );
        }
    }
}

/// Show a queued project-switch dialog during render (mirrors quit flow).
pub fn maybe_show_pending_project_switch_dialog(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    if !workspace.pending_project_switch_confirm {
        return;
    }
    workspace.pending_project_switch_confirm = false;
    let Some(root) = workspace.pending_project_switch.take() else {
        return;
    };
    if window.has_active_dialog(cx) {
        workspace.pending_project_switch = Some(root);
        workspace.pending_project_switch_confirm = true;
        return;
    }
    let dirty = workspace.has_dirty_tabs(cx);
    show_switch_project_dialog(root, workspace.registry().clone(), dirty, window, cx);
}

/// Show a queued Close Project dialog during render (mirrors switch flow).
pub fn maybe_show_pending_project_close_dialog(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    if !workspace.pending_project_close_confirm {
        return;
    }
    workspace.pending_project_close_confirm = false;
    if window.has_active_dialog(cx) {
        workspace.pending_project_close_confirm = true;
        return;
    }
    let dirty = workspace.has_dirty_project_tabs(cx);
    let live = live_project_connection_count(workspace.registry(), cx);
    match pending_close_kind(live, dirty) {
        // This runs inside Workspace::render. Do not Entity::read/update Workspace
        // here — GPUI panics with "cannot read Workspace while it is already being updated".
        PendingCloseKind::Immediate => close_project_in_workspace(workspace, window, cx),
        PendingCloseKind::Dialog => {
            show_close_project_dialog(workspace.registry().clone(), dirty, window, cx);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingCloseKind {
    Immediate,
    Dialog,
}

fn pending_close_kind(live: usize, dirty: bool) -> PendingCloseKind {
    if live > 0 || dirty {
        PendingCloseKind::Dialog
    } else {
        PendingCloseKind::Immediate
    }
}

fn project_teardown_description(
    empty: &str,
    verb_ing: &str,
    live: usize,
    dirty: bool,
) -> SharedString {
    match (live, dirty) {
        (0, false) => empty.into(),
        (1, false) => format!("You have 1 live connection. {verb_ing} will disconnect it.").into(),
        (n, false) => {
            format!("You have {n} live connections. {verb_ing} will disconnect them all.").into()
        }
        (0, true) => format!("You have unsaved query tabs. {verb_ing} will close them.").into(),
        (1, true) => format!(
            "You have 1 live connection and unsaved query tabs. {verb_ing} will disconnect and close them."
        )
        .into(),
        (n, true) => format!(
            "You have {n} live connections and unsaved query tabs. {verb_ing} will disconnect and close them."
        )
        .into(),
    }
}

fn show_switch_project_dialog(
    root: PathBuf,
    registry: Entity<ConnectionRegistry>,
    dirty: bool,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    let live = live_connection_count(&registry, cx);
    let description =
        project_teardown_description("Switch to another project?", "Switching", live, dirty);
    let root_for_ok = root.clone();

    window.open_alert_dialog(cx, move |alert, _window, cx| {
        let switch_btn = Button::new("switch-project-confirm")
            .label("Switch")
            .primary()
            .bg(cx.theme().red)
            .border_color(cx.theme().red)
            .text_color(cx.theme().primary_foreground);
        alert
            .title("Switch project?")
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("switch-cancel").outline().label("Cancel")),
                    )
                    .child(DialogAction::new().child(switch_btn)),
            )
            .on_ok({
                let root = root_for_ok.clone();
                move |_, window, cx| {
                    complete_project_switch_in_window(root.clone(), window, cx);
                    true
                }
            })
            .on_cancel(|_, _, _| true)
    });
}

fn show_close_project_dialog(
    registry: Entity<ConnectionRegistry>,
    dirty: bool,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    let live = live_project_connection_count(&registry, cx);
    let description =
        project_teardown_description("Close the current project?", "Closing", live, dirty);

    window.open_alert_dialog(cx, move |alert, _window, cx| {
        let close_btn = Button::new("close-project-confirm")
            .label("Close")
            .primary()
            .bg(cx.theme().red)
            .border_color(cx.theme().red)
            .text_color(cx.theme().primary_foreground);
        alert
            .title("Close project?")
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("close-project-cancel")
                                .outline()
                                .label("Cancel"),
                        ),
                    )
                    .child(DialogAction::new().child(close_btn)),
            )
            .on_ok(|_, window, cx| {
                complete_close_project_in_window(window, cx);
                true
            })
            .on_cancel(|_, _, _| true)
    });
}

/// Disconnect live connections, reset tabs, bind the new project.
pub fn complete_project_switch_in_window(root: PathBuf, window: &mut Window, cx: &mut App) {
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
        return;
    };
    if is_same_project(&root, cx) {
        return;
    }

    let registry = ws.read(cx).registry().clone();
    quit::disconnect_all(&registry, cx);
    if live_connection_count(&registry, cx) > 0 {
        notify::push_error(
            cx,
            "Switch project",
            "Could not disconnect all connections. Project was not changed.",
        );
        return;
    }

    ws.update(cx, |workspace, cx| {
        workspace.close_all_tabs(window, cx);
        workspace.sync_tab_manager_from_dock(cx);
    });

    bind_project(&root, &registry, cx);
    prefs::record_opened_project(root.clone(), cx);

    ws.update(cx, |workspace, cx| {
        workspace.apply_opened_project(root, cx);
    });
    cx.refresh_windows();
}

/// Disconnect live connections, reset tabs, and unbind the current project.
///
/// Dialog callbacks use this. Idle close from render must call
/// [`close_project_in_workspace`] instead — this re-enters the Workspace entity.
pub fn complete_close_project_in_window(window: &mut Window, cx: &mut App) {
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
        return;
    };
    ws.update(cx, |workspace, cx| {
        close_project_in_workspace(workspace, window, cx);
    });
}

fn close_project_in_workspace(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    if !has_open_project(cx) {
        return;
    }

    let registry = workspace.registry().clone();
    quit::disconnect_project_owned(&registry, cx);
    if live_project_connection_count(&registry, cx) > 0 {
        notify::push_error(
            cx,
            "Close project",
            "Could not disconnect project connections. Project was not closed.",
        );
        return;
    }

    workspace.close_project_tabs(window, cx);
    workspace.sync_tab_manager_from_dock(cx);
    unbind_project(&registry, cx);
    prefs::clear_last_opened_project(cx);
    workspace.apply_closed_project(cx);
    cx.refresh_windows();
}

fn bind_project(root: &Path, registry: &Entity<ConnectionRegistry>, cx: &mut App) {
    match ProjectContext::load(root.to_path_buf()) {
        Ok(ctx) => {
            cx.set_global(ctx.clone());
            apply_project_settings(&ctx.snapshot.manifest, cx);
        }
        Err(e) => {
            notify::push_error(cx, "Open Project", format!("Failed to load project: {e:#}"));
            return;
        }
    }

    install_reload_watcher(root.to_path_buf(), cx);
    cx.set_global(RegistryRef(registry.clone()));
    reload_from_disk(root, registry, cx);
}

fn unbind_project(registry: &Entity<ConnectionRegistry>, cx: &mut App) {
    stop_reload_watcher(cx);
    if cx.has_global::<ProjectContext>() {
        let _ = cx.remove_global::<ProjectContext>();
    }
    cx.update_global(|pv: &mut ProjectVars, _| pv.vars.clear());
    cx.update_global(|store: &mut QueryStore, _| store.clear_project());
    registry.update(cx, |reg, cx| {
        reg.remove_project_owned(cx);
    });
}

fn is_same_project(root: &Path, cx: &App) -> bool {
    cx.try_global::<ProjectRoot>()
        .is_some_and(|current| current.0 == root)
}

fn switch_needs_confirm(ws: &Entity<Workspace>, cx: &App) -> bool {
    let registry = ws.read(cx).registry();
    if live_connection_count(registry, cx) > 0 {
        return true;
    }
    ws.read(cx).has_dirty_tabs(cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    #[test]
    fn idle_close_skips_confirm_dialog() {
        // Immediate close is dispatched from Workspace::render; it must complete
        // via close_project_in_workspace (&mut Workspace), not Entity::read.
        assert_eq!(pending_close_kind(0, false), PendingCloseKind::Immediate);
    }

    #[test]
    fn close_with_live_connection_uses_confirm_dialog() {
        assert_eq!(pending_close_kind(1, false), PendingCloseKind::Dialog);
    }

    #[test]
    fn workspace_local_live_connections_do_not_confirm_close() {
        assert_eq!(
            pending_close_kind(
                project_owned_live_count(&[("ws-template:abc", true)]),
                false
            ),
            PendingCloseKind::Immediate
        );
    }

    #[test]
    fn mixed_live_connections_confirm_only_for_project_owned() {
        assert_eq!(
            project_owned_live_count(&[("ws-template:abc", true), ("local/northwind", true),]),
            1
        );
    }

    #[test]
    fn close_description_when_idle() {
        assert_eq!(
            project_teardown_description("Close the current project?", "Closing", 0, false)
                .as_ref(),
            "Close the current project?"
        );
    }

    #[test]
    fn close_description_with_live_connections_and_dirty_tabs() {
        assert_eq!(
            project_teardown_description("Close the current project?", "Closing", 2, true).as_ref(),
            "You have 2 live connections and unsaved query tabs. Closing will disconnect and close them."
        );
    }

    fn project_owned_live_count(rows: &[(&str, bool)]) -> usize {
        rows.iter()
            .filter(|(id, live)| *live && !ConnectionId::from_key(id).is_workspace_local())
            .count()
    }
}
