//! Shared fixtures: isolated process env, app globals, and a Workspace window.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use std::{fs, thread};

use based_desktop::app::paths::AppPaths;
use based_desktop::connection::{ConnectionConfig, ConnectionId, open_connection};
use based_desktop::db::Tokio;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::workspace::tabs::SessionSnapshot;
use based_desktop::workspace::{TabSpec, Workspace};
use based_desktop::{init_globals, query_store, storage};
use based_sqlite::config::SqliteConfig;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Focusable, Point, TestAppContext,
    WindowBounds, WindowHandle, WindowOptions, base::Root, px, size,
};
use tokio::runtime::Builder;

/// Per-process scratch dir for fixtures that need no cleanup (metadata DBs, empty trees).
fn sandbox() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        fs::create_dir_all(dir.join("personal")).expect("personal dir");
        dir
    })
}

/// Initialise app globals over `personal_root` with isolated prefs and metadata store.
pub fn init_app(cx: &mut TestAppContext, personal_root: &Path) {
    let dir = sandbox().clone();
    let unique = uuid::Uuid::new_v4();
    // Tokio workers (metadata store, session save) wake GPUI tasks from other threads.
    cx.executor().allow_parking();
    cx.update(|cx: &mut App| {
        cx.set_global(AppPaths {
            personal_root: personal_root.to_path_buf(),
            prefs_path: dir.join(format!("prefs-{unique}.toml")),
        });
        init_globals(cx);
        storage::init_at(cx, &dir.join(format!("meta-{unique}.db"))).expect("metadata store");
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

/// Initialise the app, then open a workspace window.
pub fn open_with(
    cx: &mut TestAppContext,
    workspace_env: WorkspaceEnv,
) -> (AnyWindowHandle, Entity<Workspace>) {
    init_app(cx, &workspace_env.personal_root);
    open_window(cx, workspace_env)
}

/// Open another production [`Workspace`] window (1200×800) in an already initialised app.
///
/// Sharing the app means sharing its metadata store: a second window is a relaunch
/// as far as session restore is concerned.
pub fn open_window(
    cx: &mut TestAppContext,
    workspace_env: WorkspaceEnv,
) -> (AnyWindowHandle, Entity<Workspace>) {
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

/// Create a SQLite file at `path` and run `statements` against it.
pub fn seed_sqlite(path: &Path, statements: &[&str]) {
    use sqlx::{AssertSqlSafe, Connection, SqliteConnection, sqlite::SqliteConnectOptions};
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("seed runtime");
    rt.block_on(async {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let mut conn = SqliteConnection::connect_with(&opts)
            .await
            .expect("seed db");
        for sql in statements {
            sqlx::query(AssertSqlSafe(*sql))
                .execute(&mut conn)
                .await
                .expect("seed sql");
        }
    });
}

/// Open the SQLite file at `path` and attach it to the workspace as a live connection.
pub fn connect_sqlite(
    cx: &mut TestAppContext,
    ws: &Entity<Workspace>,
    id: &str,
    path: &Path,
) -> ConnectionId {
    let config = ConnectionConfig::SQLite(SqliteConfig {
        label: id.into(),
        path: path.to_path_buf(),
        read_only: false,
        pragma: None,
    });
    let task = cx.update(|cx| open_connection(config.clone(), cx));
    let opened = cx
        .foreground_executor()
        .block_test(task)
        .expect("open sqlite");
    let conn_id = ConnectionId(id.into());
    let session = conn_id.clone();
    ws.update(cx, |ws, cx| {
        ws.attach_wizard_session(config, opened, session, vec![], false, cx)
    });
    settle(cx);
    conn_id
}

/// Poll until `done` holds: Tokio work finishes off the test scheduler's thread.
pub fn wait_until(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..300 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("condition not reached in time");
}

/// The session snapshot as currently stored (what the next launch would restore).
pub fn saved_session(cx: &mut TestAppContext) -> SessionSnapshot {
    cx.update(|cx| {
        let store = storage::store(cx);
        Tokio::handle(cx).block_on(SessionSnapshot::load(&store))
    })
}

/// Store `snapshot` as if a previous run had saved it.
pub fn store_session(cx: &mut TestAppContext, snapshot: &SessionSnapshot) {
    cx.update(|cx| {
        let store = storage::store(cx);
        Tokio::handle(cx)
            .block_on(snapshot.save(&store))
            .expect("save session");
    });
}

/// An object-info tab spec on a placeholder connection (needs no live connection).
pub fn info(name: &str) -> TabSpec {
    TabSpec::ObjectInfo {
        conn_id: ConnectionId("t".into()),
        object_name: name.into(),
        kind_label: "Trigger".into(),
    }
}
