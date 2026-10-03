//! Application window launch: open the main workspace.

use gpui_kit::component::Root;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, BorrowAppContext, WindowBounds, WindowOptions, px, size,
};

use super::shell::{self, APP_NAME};
use crate::workspace::{PopOutManager, Workspace, WorkspaceRef};

/// Open the main workspace window (no-op if already open).
pub fn open_main_workspace(cx: &mut App) -> anyhow::Result<()> {
    if cx.try_global::<WorkspaceRef>().is_some() {
        return Ok(());
    }

    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.0), px(800.0)), cx)),
            titlebar: Some(shell::titled_titlebar(APP_NAME)),
            ..shell::identified_window_options()
        },
        |window, cx| {
            window.set_window_title(APP_NAME);
            let workspace = cx.new(|cx| Workspace::new(window, cx));
            cx.set_global(WorkspaceRef(workspace.clone()));
            cx.new(|cx| Root::new(workspace, window, cx))
        },
    )?;

    let any: AnyWindowHandle = opened.into();
    cx.update_global(|manager: &mut PopOutManager, _| {
        manager.main_window_id = Some(any.window_id());
    });
    Ok(())
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
