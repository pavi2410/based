//! Tab strip lifecycle: Home respawn, dedup, pinning.
//!
//! Assertions use tab counts on the settled state; specs are re-inferred from
//! panels on dock sync, so they are not a stable identity for every tab kind.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::tabs::CloseScope;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::strip::{open_tab, specs};
use crate::common::{
    connect_sqlite, connect_sqlite_and_open, info, open_workspace, seed_sqlite, settle, wait_until,
};

/// Number of distinct tab panels (see `one_manager_entry_per_panel` for why not `tabs.len()`).
fn tab_count(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> usize {
    ws.read_with(cx, |ws, cx| {
        let mut ids: Vec<_> = ws
            .tab_manager()
            .read(cx)
            .tabs
            .iter()
            .map(|t| t.view().entity_id())
            .collect();
        ids.sort();
        ids.dedup();
        ids.len()
    })
}

fn has_home_only(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> bool {
    ws.read_with(cx, |ws, cx| {
        let tabs = &ws.tab_manager().read(cx).tabs;
        tabs.len() == 1 && tabs[0].spec == TabSpec::Home
    })
}

fn close_active(win: AnyWindowHandle, ws: &Entity<Workspace>, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.close_tabs(CloseScope::Active, window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

#[gpui_kit::test]
fn starts_with_home_tab(cx: &mut TestAppContext) {
    let (_win, ws) = open_workspace(cx);
    assert!(has_home_only(&ws, cx));
}

#[gpui_kit::test]
fn distinct_specs_open_distinct_tabs(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("b"), cx);
    assert_eq!(tab_count(&ws, cx), 3, "Home + two tabs");
}

// Dock sync must not overwrite specs of panels it cannot infer, or dedup stops matching.
#[gpui_kit::test]
fn opening_same_spec_twice_does_not_duplicate(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("a"), cx);
    assert_eq!(tab_count(&ws, cx), 2, "Home + one tab");
}

// Dock sync registers the panel before `open_or_focus` runs; both must share one entry.
#[gpui_kit::test]
fn one_manager_entry_per_panel(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    let entries = ws.read_with(cx, |ws, cx| ws.tab_manager().read(cx).tabs.len());
    assert_eq!(entries, 2, "Home + one tab");
}

#[gpui_kit::test]
fn reopening_a_tab_focuses_it(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    open_tab(win, &ws, info("b"), cx);
    open_tab(win, &ws, info("a"), cx);
    let a_is_active = ws.read_with(cx, |ws, cx| {
        let a = ws
            .tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find(|t| t.spec == info("a"))
            .expect("tab a");
        ws.active_center_panel_id(cx) == Some(a.view().entity_id())
    });
    assert!(a_is_active, "re-opening should activate the existing tab");
    assert_eq!(tab_count(&ws, cx), 3);
}

#[gpui_kit::test]
fn closing_active_tab_removes_it(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    assert_eq!(tab_count(&ws, cx), 2);
    close_active(win, &ws, cx);
    assert_eq!(tab_count(&ws, cx), 1);
}

#[gpui_kit::test]
fn closing_last_tab_respawns_home(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    close_active(win, &ws, cx);
    assert!(has_home_only(&ws, cx), "closing Home respawns Home");
}

#[gpui_kit::test]
fn cmd_w_closes_active_tab(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| window.press("cmd-w", cx))
        .unwrap();
    settle(cx);
    assert_eq!(tab_count(&ws, cx), 1);
}

#[gpui_kit::test]
fn pinned_tab_cannot_be_closed(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            let id = ws.active_center_panel_id(cx).expect("active panel");
            ws.toggle_pin_tab(id, cx);
            assert!(ws.is_tab_pinned(id, cx));
            ws.close_tabs(CloseScope::Tab(id), window, cx);
        });
    })
    .unwrap();
    settle(cx);
    assert_eq!(tab_count(&ws, cx), 2, "pinned tab must survive close");
}

// Activation happens in the dock; the manager must mirror it.
#[gpui_kit::test]
fn tab_manager_active_follows_dock(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open_tab(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.show_home(window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
    let active_is_home = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .active_tab()
            .is_some_and(|t| t.spec == TabSpec::Home)
    });
    assert!(
        active_is_home,
        "TabManager active tab should follow the dock"
    );
}

fn has_query_tab_for(ws: &Entity<Workspace>, id: &str, cx: &mut TestAppContext) -> bool {
    specs(ws, cx)
        .iter()
        .any(|s| matches!(s, TabSpec::QueryEditor { conn_id, .. } if *conn_id == ConnectionId::parse(id)))
}

#[gpui_kit::test]
fn opening_a_connection_keeps_open_tabs(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = (dir.path().join("a.db"), dir.path().join("b.db"));
    seed_sqlite(&a, &["create table t (id integer primary key)"]);
    seed_sqlite(&b, &["create table u (id integer primary key)"]);
    let (win, ws) = open_workspace(cx);
    connect_sqlite(cx, &ws, "a", &a);
    open_tab(win, &ws, info("kept"), cx);

    connect_sqlite_and_open(cx, &ws, "b", &b);
    wait_until(cx, |cx| {
        cx.update_window(win, |_, window, _| window.refresh()).ok();
        has_query_tab_for(&ws, "b", cx)
    });

    assert!(
        specs(&ws, cx).contains(&info("kept")),
        "tabs open before the connection must survive it"
    );
}

#[gpui_kit::test]
fn first_connection_replaces_a_lone_home_tab(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.db");
    seed_sqlite(&a, &["create table t (id integer primary key)"]);
    let (win, ws) = open_workspace(cx);
    assert!(has_home_only(&ws, cx));

    connect_sqlite_and_open(cx, &ws, "a", &a);
    wait_until(cx, |cx| {
        cx.update_window(win, |_, window, _| window.refresh()).ok();
        has_query_tab_for(&ws, "a", cx)
    });

    assert!(!specs(&ws, cx).contains(&TabSpec::Home));
    assert_eq!(tab_count(&ws, cx), 1);
}
