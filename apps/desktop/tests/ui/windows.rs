//! Several workspace windows in one app, each over its own project.

use std::fs;
use std::path::Path;

use based_desktop::app::launch;
use based_desktop::project::open::complete_project_switch_in_window;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::workspace::tabs::{SessionScope, SessionSnapshot, SessionTab};
use based_desktop::workspace::{QueryEditorInit, TabSpec, Workspaces};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext as _, TestAppContext};

use crate::common::strip::{open_tab, specs};
use crate::common::{
    connect_sqlite, info, init_app, no_project_env, open_window, saved_session_in, seed_sqlite,
    settle, store_session_in, wait_until,
};

fn project_env(root: &Path) -> WorkspaceEnv {
    fs::create_dir_all(root.join(".based")).unwrap();
    fs::write(
        root.join(".based/project.toml"),
        "schema_version = 1\nname = \"demo\"\n",
    )
    .unwrap();
    WorkspaceEnv {
        project_dir: Some(root.to_path_buf()),
        ..no_project_env()
    }
}

fn history_len(win: AnyWindowHandle, cx: &mut TestAppContext) -> usize {
    cx.update(|cx| {
        Workspaces::project(win.window_id(), cx).map_or(0, |p| p.queries.history.recent(10).len())
    })
}

#[gpui_kit::test]
fn each_window_has_its_own_project(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    init_app(cx, &no_project_env().personal_root);
    let (win_a, ws_a) = open_window(cx, project_env(a.path()));
    let (win_b, _ws_b) = open_window(cx, project_env(b.path()));

    cx.update(|cx| {
        assert_eq!(
            Workspaces::window_with_project(a.path(), cx),
            Some(win_a.window_id())
        );
        assert_eq!(
            Workspaces::window_with_project(b.path(), cx),
            Some(win_b.window_id())
        );
    });

    // A panel in A finds A's project even though B opened last.
    open_tab(win_a, &ws_a, info("t"), cx);
    let panel = ws_a.read_with(cx, |ws, cx| ws.active_center_panel_id(cx).expect("panel"));
    let root = cx.update(|cx| {
        Workspaces::project_for_panel(panel, cx).and_then(|p| p.root().map(ToOwned::to_owned))
    });
    assert_eq!(root, Some(fs::canonicalize(a.path()).unwrap()));
}

#[gpui_kit::test]
fn a_query_run_in_one_window_is_recorded_in_that_project(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = a.path().join("demo.db");
    seed_sqlite(&db, &["CREATE TABLE t (x INTEGER)"]);
    init_app(cx, &no_project_env().personal_root);
    let (win_a, ws_a) = open_window(cx, project_env(a.path()));
    let conn = connect_sqlite(cx, &ws_a, "demo", &db);
    let (win_b, _ws_b) = open_window(cx, project_env(b.path()));

    let spec = TabSpec::QueryEditor {
        conn_id: conn,
        init: QueryEditorInit {
            sql: Some("SELECT 1".into()),
            auto_run: true,
            saved: None,
            dirty: false,
        },
    };
    open_tab(win_a, &ws_a, spec, cx);
    wait_until(cx, |cx| history_len(win_a, cx) == 1);
    assert_eq!(history_len(win_b, cx), 0, "B's history is untouched");
}

#[gpui_kit::test]
fn opening_an_open_project_reuses_its_window(cx: &mut TestAppContext) {
    let a = tempfile::tempdir().unwrap();
    init_app(cx, &no_project_env().personal_root);
    let _ = open_window(cx, project_env(a.path()));
    let before = cx.update(|cx| cx.windows().len());
    cx.update(|cx| launch::open_project_window(a.path().to_path_buf(), cx))
        .unwrap();
    assert_eq!(cx.update(|cx| cx.windows().len()), before);
}

#[gpui_kit::test]
fn closing_one_window_leaves_the_other(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    init_app(cx, &no_project_env().personal_root);
    let (win_a, _) = open_window(cx, project_env(a.path()));
    let (win_b, _) = open_window(cx, project_env(b.path()));

    cx.update(|cx| launch::on_window_closed(win_a.window_id(), cx));
    cx.update(|cx| {
        assert!(Workspaces::project(win_a.window_id(), cx).is_none());
        assert!(Workspaces::project(win_b.window_id(), cx).is_some());
        assert!(!Workspaces::is_empty(cx));
    });
}

fn saved_has(cx: &mut TestAppContext, root: &Path, spec: &TabSpec) -> bool {
    saved_session_in(cx, &SessionScope::for_project(Some(root)))
        .tabs
        .iter()
        .any(|t| t.spec == *spec)
}

#[gpui_kit::test]
fn switching_project_in_a_window_keeps_both_sessions(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let env_b = project_env(b.path());
    init_app(cx, &env_b.personal_root);
    let saved_b = SessionSnapshot {
        tabs: vec![SessionTab::new(info("b"))],
        ..Default::default()
    };
    store_session_in(cx, &SessionScope::for_project(Some(b.path())), &saved_b);

    let (win, ws) = open_window(cx, project_env(a.path()));
    open_tab(win, &ws, info("a"), cx);
    wait_until(cx, |cx| saved_has(cx, a.path(), &info("a")));

    let root_b = b.path().to_path_buf();
    cx.update_window(win, |_, window, cx| {
        complete_project_switch_in_window(root_b, window, cx);
    })
    .unwrap();
    cx.update_window(win, |_, window, cx| window.render_frame(cx))
        .unwrap();
    settle(cx);

    assert!(specs(&ws, cx).contains(&info("b")), "B's tabs come back");
    assert!(!specs(&ws, cx).contains(&info("a")));
    assert!(saved_has(cx, a.path(), &info("a")), "A's session is kept");
    assert!(
        saved_has(cx, b.path(), &info("b")),
        "B's session is not overwritten"
    );
}
