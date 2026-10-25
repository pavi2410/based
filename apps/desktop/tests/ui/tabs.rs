//! Tab strip lifecycle: Home respawn, dedup, pinning.
//!
//! Assertions use tab counts on the settled state; specs are re-inferred from
//! panels on dock sync, so they are not a stable identity for every tab kind.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{open_workspace, settle};

fn info(name: &str) -> TabSpec {
    TabSpec::ObjectInfo {
        conn_id: ConnectionId("t".into()),
        object_name: name.into(),
        kind_label: "Trigger".into(),
    }
}

/// Number of distinct tab panels (see `one_manager_entry_per_panel` for why not `tabs.len()`).
fn tab_count(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> usize {
    ws.read_with(cx, |ws, cx| {
        let mut ids: Vec<_> = ws
            .tab_manager()
            .read(cx)
            .tabs
            .iter()
            .map(|t| t.view.entity_id())
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

fn open(win: AnyWindowHandle, ws: &Entity<Workspace>, spec: TabSpec, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| {
        let opened = ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec.clone(), window, cx));
        assert!(opened, "dispatch_open_tab refused {spec:?}");
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

fn close_active(win: AnyWindowHandle, ws: &Entity<Workspace>, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.close_active_center_tab(window, cx));
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
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("b"), cx);
    assert_eq!(tab_count(&ws, cx), 3, "Home + two tabs");
}

// `infer_tab_spec` maps ObjectInfo panels to `Builtin`, so dock sync overwrites
// the spec and `tab_has_spec` no longer matches the second open.
#[gpui_kit::test]
#[ignore = "known bug: infer_tab_spec loses ObjectInfo/DataViewer/Inspector specs"]
fn opening_same_spec_twice_does_not_duplicate(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("a"), cx);
    assert_eq!(tab_count(&ws, cx), 2, "Home + one tab");
}

// Dock sync registers the panel with an inferred `Builtin` spec first, then
// `open_or_focus` pushes a second entry for the same view.
#[gpui_kit::test]
#[ignore = "known bug: TabManager gets two entries for one ObjectInfo panel"]
fn one_manager_entry_per_panel(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    let entries = ws.read_with(cx, |ws, cx| ws.tab_manager().read(cx).tabs.len());
    assert_eq!(entries, 2, "Home + one tab");
}

#[gpui_kit::test]
fn closing_active_tab_removes_it(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
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
    open(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| window.press("cmd-w", cx))
        .unwrap();
    settle(cx);
    assert_eq!(tab_count(&ws, cx), 1);
}

#[gpui_kit::test]
fn pinned_tab_cannot_be_closed(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            let id = ws.active_center_panel_id(cx).expect("active panel");
            ws.toggle_pin_tab(id, cx);
            assert!(ws.is_tab_pinned(id, cx));
            ws.close_center_panel(id, window, cx);
        });
    })
    .unwrap();
    settle(cx);
    assert_eq!(tab_count(&ws, cx), 2, "pinned tab must survive close");
}

// `sync_tab_manager_from_dock` prefers the manager's own active tab over the
// dock's live one, so activating Home leaves the manager on the old tab.
#[gpui_kit::test]
#[ignore = "known bug: TabManager.active_idx does not follow dock activation"]
fn tab_manager_active_follows_dock(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
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
