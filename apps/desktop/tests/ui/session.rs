//! Session save and restore across launches.
//!
//! A "launch" is a Workspace window over the same app (and metadata store).

use std::fs;
use std::path::Path;

use based_desktop::connection::ConnectionId;
use based_desktop::db::Tokio;
use based_desktop::storage;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::workspace::panels::query_editor::QueryEditorPanel;
use based_desktop::workspace::tabs::{CloseScope, SessionScope, SessionSnapshot, SessionTab};
use based_desktop::workspace::{QueryEditorInit, TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};

use crate::common::strip::{active_spec, open_tab, specs};
use crate::common::{
    connect_sqlite, info, init_app, no_project_env, open_window, open_with, open_workspace,
    saved_session, saved_session_in, saved_specs, seed_sqlite, settle, store_session, wait_until,
};

/// Wait for the async writer to store a session containing `spec`.
fn wait_saved(cx: &mut TestAppContext, spec: &TabSpec) {
    wait_until(cx, |cx| saved_specs(cx).contains(spec));
}

fn snapshot(specs: impl IntoIterator<Item = TabSpec>) -> SessionSnapshot {
    SessionSnapshot {
        tabs: specs.into_iter().map(SessionTab::new).collect(),
        ..Default::default()
    }
}

#[gpui_kit::test]
fn open_tabs_are_saved(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("b"), cx);
    wait_saved(cx, &info("b"));
    let saved = saved_specs(cx);
    assert!(saved.contains(&TabSpec::Home) && saved.contains(&info("a")));
}

#[gpui_kit::test]
fn next_launch_restores_tabs(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("b"), cx);
    wait_saved(cx, &info("b"));

    let (_win2, ws2) = open_window(cx, no_project_env());
    let restored = specs(&ws2, cx);
    let a = restored.iter().position(|s| *s == info("a"));
    let b = restored.iter().position(|s| *s == info("b"));
    assert!(a.is_some() && b.is_some(), "{restored:?}");
    assert!(a < b, "restored in saved order: {restored:?}");
    assert_eq!(
        restored.iter().filter(|s| **s == TabSpec::Home).count(),
        1,
        "Home is not duplicated"
    );
}

#[gpui_kit::test]
fn next_launch_restores_the_active_tab(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("b"), cx);
    open_tab(win, &ws, info("a"), cx); // re-open focuses `a`
    wait_until(cx, |cx| {
        let s = saved_session(cx);
        s.active.and_then(|i| s.tabs.get(i)).map(|t| &t.spec) == Some(&info("a"))
    });

    let (_win2, ws2) = open_window(cx, no_project_env());
    assert_eq!(active_spec(&ws2, cx), Some(info("a")));
}

#[gpui_kit::test]
fn next_launch_restores_pinned_tabs(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    ws.update(cx, |ws, cx| {
        let id = ws.active_center_panel_id(cx).expect("active panel");
        ws.toggle_pin_tab(id, cx);
    });
    wait_until(cx, |cx| {
        saved_session(cx)
            .tabs
            .iter()
            .any(|t| t.pinned && t.spec == info("a"))
    });

    let (_win2, ws2) = open_window(cx, no_project_env());
    let pinned = ws2.read_with(cx, |ws, cx| ws.tab_manager().read(cx).pinned_specs());
    assert_eq!(pinned, [info("a")]);
}

#[gpui_kit::test]
fn tabs_for_unavailable_connections_survive_a_save(cx: &mut TestAppContext) {
    // A previous run left a data viewer for a connection that is not (yet) live.
    let gone = TabSpec::DataViewer {
        conn_id: ConnectionId::parse("gone"),
        object: based_core::ObjectRef::bare("users"),
        filter: None,
    };
    let (win, ws) = open_workspace(cx);
    store_session(cx, &snapshot([TabSpec::Home, gone.clone()]));
    let (win2, ws2) = open_window(cx, no_project_env());
    assert!(
        !specs(&ws2, cx).contains(&gone),
        "cannot open without its connection"
    );

    // Any later change re-saves the session; the pending tab must not be dropped.
    open_tab(win2, &ws2, info("a"), cx);
    wait_saved(cx, &info("a"));
    assert!(saved_specs(cx).contains(&gone), "pending tab lost on save");
    let _ = (win, ws);
}

/// A previous run saved one query tab for SQLite connection `demo`; a new
/// launch restores it once `demo` is connected.
struct Restored {
    win: AnyWindowHandle,
    ws: Entity<Workspace>,
    _dir: tempfile::TempDir,
}

fn restored_query(cx: &mut TestAppContext) -> Restored {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, &["create table t (id integer)"]);
    let spec = TabSpec::QueryEditor {
        conn_id: ConnectionId::parse("demo"),
        init: QueryEditorInit {
            sql: Some("select 1".into()),
            auto_run: false,
            saved: None,
            dirty: false,
        },
    };
    let _first = open_workspace(cx);
    store_session(cx, &snapshot([TabSpec::Home, spec]));
    let (win, ws) = open_window(cx, no_project_env());
    connect_sqlite(cx, &ws, "demo", &db);
    settle(cx);
    Restored { win, ws, _dir: dir }
}

impl Restored {
    fn editor(&self, cx: &mut TestAppContext) -> Entity<QueryEditorPanel> {
        self.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .tabs
                .iter()
                .find_map(|t| t.downcast::<QueryEditorPanel>())
                .expect("restored query tab")
        })
    }

    /// Change something else so the session is written, then read it back.
    fn save_and_read(&self, cx: &mut TestAppContext) -> Vec<TabSpec> {
        open_tab(self.win, &self.ws, info("z"), cx);
        wait_saved(cx, &info("z"));
        saved_specs(cx)
    }
}

fn query_tabs(specs: &[TabSpec]) -> usize {
    specs
        .iter()
        .filter(|s| matches!(s, TabSpec::QueryEditor { .. }))
        .count()
}

#[gpui_kit::test]
fn editing_a_restored_query_saves_it_once(cx: &mut TestAppContext) {
    let r = restored_query(cx);
    let editor = r.editor(cx);
    cx.update_window(r.win, |_, window, cx| {
        editor.update(cx, |p, cx| p.focus_editor(window, cx));
        window.input(" -- edited", cx);
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
    let saved = r.save_and_read(cx);
    assert_eq!(query_tabs(&saved), 1, "{saved:?}");
}

#[gpui_kit::test]
fn a_closed_restored_tab_does_not_come_back(cx: &mut TestAppContext) {
    let r = restored_query(cx);
    let editor = r.editor(cx);
    let id = editor.entity_id();
    cx.update_window(r.win, |_, window, cx| {
        r.ws.update(cx, |ws, cx| ws.close_tabs(CloseScope::Tab(id), window, cx))
    })
    .unwrap();
    settle(cx);
    let saved = r.save_and_read(cx);
    assert_eq!(query_tabs(&saved), 0, "{saved:?}");
}

#[gpui_kit::test]
fn pinning_one_of_two_equal_tabs_pins_only_that_one(cx: &mut TestAppContext) {
    // Two blank editors on one connection have equal specs.
    let r = restored_query(cx);
    let blank = TabSpec::blank_query_editor(ConnectionId::parse("demo"));
    open_tab(r.win, &r.ws, blank.clone(), cx);
    open_tab(r.win, &r.ws, blank.clone(), cx);
    r.ws.update(cx, |ws, cx| {
        let id = ws.active_center_panel_id(cx).expect("active panel");
        ws.toggle_pin_tab(id, cx);
    });
    wait_until(cx, |cx| saved_session(cx).tabs.iter().any(|t| t.pinned));
    let pinned = saved_session(cx).tabs.iter().filter(|t| t.pinned).count();
    assert_eq!(pinned, 1);

    // Next launch: both blank editors come back, and only one is pinned.
    let (_w3, ws3) = open_window(cx, no_project_env());
    connect_sqlite(cx, &ws3, "demo", r._dir.path().join("demo.db").as_path());
    settle(cx);
    let tabs = ws3.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            // The new editors (they open with the same default SQL), not the
            // restored `select 1` one.
            .filter(|t| {
                matches!(&t.spec, TabSpec::QueryEditor { init, .. }
                    if init.sql.as_deref() != Some("select 1"))
            })
            .map(|t| t.pinned)
            .collect::<Vec<_>>()
    });
    assert_eq!(tabs.len(), 2, "both blank editors restored: {tabs:?}");
    assert_eq!(tabs.iter().filter(|p| **p).count(), 1, "{tabs:?}");
}

fn project_env(root: &Path) -> WorkspaceEnv {
    fs::create_dir_all(root.join(".based")).unwrap();
    WorkspaceEnv {
        project_dir: Some(root.to_path_buf()),
        ..no_project_env()
    }
}

fn scope(root: &Path) -> SessionScope {
    SessionScope::for_project(Some(root))
}

#[gpui_kit::test]
fn each_project_restores_only_its_own_tabs(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (win_a, ws_a) = open_with(cx, project_env(a.path()));
    open_tab(win_a, &ws_a, info("in-a"), cx);
    wait_until(cx, |cx| {
        saved_session_in(cx, &scope(a.path()))
            .tabs
            .iter()
            .any(|t| t.spec == info("in-a"))
    });

    let (_win_b, ws_b) = open_window(cx, project_env(b.path()));
    assert!(
        !specs(&ws_b, cx).contains(&info("in-a")),
        "B has its own slot"
    );
    let (_win, ws_a2) = open_window(cx, project_env(a.path()));
    assert!(specs(&ws_a2, cx).contains(&info("in-a")));
}

#[gpui_kit::test]
fn the_session_saved_before_scopes_is_inherited_once(cx: &mut TestAppContext) {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let env_a = project_env(a.path());
    init_app(cx, &env_a.personal_root);
    cx.update(|cx| {
        let store = storage::store(cx);
        let tabs = vec![SessionTab::new(info("old"))];
        Tokio::handle(cx)
            .block_on(store.set_session_json(based_storage::OPEN_TABS, &tabs))
            .unwrap();
    });

    let (_win_a, ws_a) = open_window(cx, env_a);
    assert!(
        specs(&ws_a, cx).contains(&info("old")),
        "first project inherits it"
    );
    let (_win_b, ws_b) = open_window(cx, project_env(b.path()));
    assert!(
        !specs(&ws_b, cx).contains(&info("old")),
        "and only the first"
    );
}
