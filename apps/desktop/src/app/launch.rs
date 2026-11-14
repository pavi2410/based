//! Opening workspace windows. Every window runs in this one process and shows one
//! project (or none); opening a project that already has a window focuses it.

use std::path::PathBuf;

use gpui_kit::{App, AppContext, Bounds, WindowBounds, WindowId, WindowOptions, point, px, size};

use super::aux_windows::AuxWindows;
use super::paths::AppPaths;
use super::shell::{self, APP_NAME};
use crate::db::Tokio;
use crate::storage;
use crate::workspace::boot::WorkspaceEnv;
use crate::workspace::tabs::SessionScope;
use crate::workspace::{PopOutManager, Workspace, Workspaces};

/// How far a new window sits from the one it was opened from.
const CASCADE: f32 = 28.0;

/// Open the first workspace window (no-op if one is already open).
pub fn open_main_workspace(cx: &mut App) -> anyhow::Result<()> {
    if !Workspaces::all(cx).is_empty() {
        return Ok(());
    }
    open_workspace_window(WorkspaceEnv::from_paths(AppPaths::get(cx)), cx)
}

/// Show `root` in a window of its own: the one that already has it open, else a
/// new one.
pub fn open_project_window(root: PathBuf, cx: &mut App) -> anyhow::Result<()> {
    if let Some(window) = Workspaces::window_with_project(&root, cx) {
        activate_window(window, cx);
        return Ok(());
    }
    let env = WorkspaceEnv {
        project_dir: Some(root),
        personal_root: AppPaths::get(cx).personal_root.clone(),
    };
    open_workspace_window(env, cx)
}

/// Bring `window` to the front.
pub fn activate_window(window: WindowId, cx: &mut App) {
    if let Some(handle) = cx.windows().into_iter().find(|w| w.window_id() == window) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}

fn open_workspace_window(env: WorkspaceEnv, cx: &mut App) -> anyhow::Result<()> {
    let bounds = saved_placement(&env, cx)
        .or_else(|| cascaded_bounds(cx))
        .unwrap_or_else(|| WindowBounds::centered(size(px(1280.0), px(800.0)), cx));
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            titlebar: Some(shell::titled_titlebar(APP_NAME)),
            ..shell::identified_window_options()
        },
        cx,
        |window, cx| cx.new(|cx| Workspace::with_env(env, window, cx)),
    )?;
    Ok(())
}

/// Where this project's window was last, if that spot is still on a display
/// (a monitor may have been unplugged since).
fn saved_placement(env: &WorkspaceEnv, cx: &App) -> Option<WindowBounds> {
    let scope = SessionScope::for_project(env.project_dir.as_deref());
    let store = storage::store(cx);
    let saved = Tokio::handle(cx).block_on(scope.load_placement(&store))?;
    let center = saved.get_bounds().center();
    cx.displays()
        .iter()
        .any(|d| d.bounds().contains(&center))
        .then_some(saved)
}

/// The focused workspace window's size, shifted down and right.
fn cascaded_bounds(cx: &mut App) -> Option<WindowBounds> {
    let active = Workspaces::active_window(cx)?;
    let handle = cx.windows().into_iter().find(|w| w.window_id() == active)?;
    let current = handle.update(cx, |_, window, _| window.bounds()).ok()?;
    Some(WindowBounds::Windowed(Bounds {
        origin: current.origin + point(px(CASCADE), px(CASCADE)),
        size: current.size,
    }))
}

/// A window closed. A workspace window takes its pop-outs with it, and closing
/// the last one quits.
pub fn on_window_closed(window: WindowId, cx: &mut App) {
    let was_workspace = Workspaces::unregister(window, cx);
    PopOutManager::on_window_closed(window, cx);
    AuxWindows::on_window_closed(window, cx);
    if was_workspace && Workspaces::is_empty(cx) {
        AuxWindows::close_all(cx);
        cx.quit();
    }
}

/// Entry point after app init: open the main workspace.
pub fn spawn_initial_window(cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.update(|app| {
            if let Err(err) = open_main_workspace(app) {
                log::error!("failed to open main workspace: {err:#}");
            }
        });
    })
    .detach();
}
