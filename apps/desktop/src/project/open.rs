//! Open a `.based/` project from the GUI — in-place switch or new process.

use std::path::{Path, PathBuf};

use gpui::{App, Entity, SharedString, Window, prelude::*};
use gpui_component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
};

use crate::app::prefs;
use crate::app::quit;
use crate::connection::live_connection_count;
use crate::connection::registry::ConnectionRegistry;
use crate::project::ProjectContext;
use crate::project::discovery::resolve_project_root;
use crate::project::reload::{RegistryRef, install_reload_watcher, reload_from_disk};
use crate::workspace::Workspace;
use crate::workspace::WorkspaceRef;
use crate::workspace::notify;

use super::pick;

/// After folder pick: open in the current window (may confirm when connections/tabs are dirty).
pub fn prompt_open_project_in_window(cx: &mut App) {
    spawn_pick_and_then(cx, ProjectOpenMode::InWindow);
}

/// After folder pick: spawn a new Based process with `BASED_PROJECT_DIR`.
pub fn prompt_open_project_in_new_window(cx: &mut App) {
    spawn_pick_and_then(cx, ProjectOpenMode::NewProcess);
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
    let exe = match std::env::current_exe() {
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
    match std::process::Command::new(exe)
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

fn show_switch_project_dialog(
    root: PathBuf,
    registry: Entity<ConnectionRegistry>,
    dirty: bool,
    window: &mut Window,
    cx: &mut gpui::Context<Workspace>,
) {
    let live = live_connection_count(&registry, cx);
    let description: SharedString = match (live, dirty) {
        (0, false) => "Switch to another project?".into(),
        (n, false) if n == 1 => {
            "You have 1 live connection. Switching will disconnect it.".into()
        }
        (n, false) => format!(
            "You have {n} live connections. Switching will disconnect them all."
        )
        .into(),
        (0, true) => "You have unsaved query tabs. Switching will close them.".into(),
        (n, true) if n == 1 => {
            "You have 1 live connection and unsaved query tabs. Switching will disconnect and close them."
                .into()
        }
        (n, true) => format!(
            "You have {n} live connections and unsaved query tabs. Switching will disconnect and close them."
        )
        .into(),
    };
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

fn bind_project(root: &Path, registry: &Entity<ConnectionRegistry>, cx: &mut App) {
    match ProjectContext::load(root.to_path_buf()) {
        Ok(ctx) => {
            cx.set_global(ctx.clone());
            crate::project::settings::apply_project_settings(&ctx.snapshot.manifest, cx);
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

fn is_same_project(root: &Path, cx: &App) -> bool {
    cx.try_global::<crate::project::ProjectRoot>()
        .is_some_and(|current| current.0 == root)
}

fn switch_needs_confirm(ws: &Entity<Workspace>, cx: &App) -> bool {
    let registry = ws.read(cx).registry();
    if live_connection_count(registry, cx) > 0 {
        return true;
    }
    ws.read(cx).has_dirty_tabs(cx)
}
