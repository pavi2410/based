//! Shared fixtures: isolated process env, app globals, and a Workspace window.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs};

use based_desktop::workspace::Workspace;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::{init_globals, query_store, storage};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Focusable, Point, TestAppContext,
    WindowBounds, WindowHandle, WindowOptions, base::Root, px, size,
};

/// Per-process scratch dir; keeps tests off the real project and `~/.config/based`.
fn sandbox() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        fs::create_dir_all(dir.join("personal")).expect("personal dir");
        // Guards code outside `Workspace` that still reads the personal root from the env.
        // SAFETY: runs once, inside `get_or_init`, before any test reads these vars.
        unsafe {
            env::set_var("XDG_CONFIG_HOME", dir.join("config"));
        }
        dir
    })
}

/// Initialise app globals and an isolated metadata store.
pub fn init_app(cx: &mut TestAppContext) {
    let dir = sandbox().clone();
    // Tokio workers (metadata store, session save) wake GPUI tasks from other threads.
    cx.executor().allow_parking();
    cx.update(|cx: &mut App| {
        init_globals(cx);
        let db = dir.join(format!("meta-{}.db", uuid::Uuid::new_v4()));
        storage::init_at(cx, &db).expect("metadata store");
        query_store::init(None, None, cx);
    });
}

/// Workspace with no project and an empty personal tree.
pub fn open_workspace(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Workspace>) {
    open_with(cx, no_project_env())
}

/// Workspace over the project at `project_dir` (a folder containing `.based/`).
pub fn open_project(
    cx: &mut TestAppContext,
    project_dir: &Path,
) -> (AnyWindowHandle, Entity<Workspace>) {
    open_with(
        cx,
        WorkspaceEnv {
            project_dir: Some(project_dir.to_path_buf()),
            ..no_project_env()
        },
    )
}

pub fn no_project_env() -> WorkspaceEnv {
    WorkspaceEnv {
        project_dir: None,
        personal_root: sandbox().join("personal"),
    }
}

/// Open the production [`Workspace`] in a 1200×800 headless window.
pub fn open_with(
    cx: &mut TestAppContext,
    workspace_env: WorkspaceEnv,
) -> (AnyWindowHandle, Entity<Workspace>) {
    init_app(cx);
    let opened = cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(1200.), px(800.)),
        };
        let (window, ws) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::with_env(workspace_env, window, cx)),
        )
        .expect("open workspace window");
        let _: WindowHandle<Root> = window.downcast().expect("Root");
        (window, ws)
    });
    // Key bindings route through the focused element.
    let (win, ws) = opened;
    cx.update_window(win, |_, window, cx| {
        ws.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
    (win, ws)
}

/// Drain deferred effects (dock layout sync, session save) so reads see settled state.
pub fn settle(cx: &mut TestAppContext) {
    cx.run_until_parked();
}
