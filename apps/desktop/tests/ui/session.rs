//! Session save and restore across launches.
//!
//! A "launch" is a Workspace window over the same app (and metadata store).

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::tabs::SessionSnapshot;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{
    info, no_project_env, open_window, open_workspace, saved_session, settle, store_session,
    wait_until,
};

fn open(win: AnyWindowHandle, ws: &Entity<Workspace>, spec: TabSpec, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| {
        assert!(ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx)));
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

fn specs(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<TabSpec> {
    ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .map(|t| t.spec.clone())
            .collect()
    })
}

fn active_spec(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Option<TabSpec> {
    ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .active_tab()
            .map(|t| t.spec.clone())
    })
}

/// Wait for the async writer to store a session containing `spec`.
fn wait_saved(cx: &mut TestAppContext, spec: &TabSpec) {
    wait_until(cx, |cx| saved_session(cx).tabs.contains(spec));
}

#[gpui_kit::test]
fn open_tabs_are_saved(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("b"), cx);
    wait_saved(cx, &info("b"));
    let saved = saved_session(cx).tabs;
    assert!(saved.contains(&TabSpec::Home) && saved.contains(&info("a")));
}

#[gpui_kit::test]
fn next_launch_restores_tabs(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("b"), cx);
    wait_saved(cx, &info("b"));

    let (_win2, ws2) = open_window(cx, no_project_env());
    let restored = specs(&ws2, cx);
    assert!(restored.contains(&info("a")), "{restored:?}");
    assert!(restored.contains(&info("b")), "{restored:?}");
    assert_eq!(
        restored.iter().filter(|s| **s == TabSpec::Home).count(),
        1,
        "Home is not duplicated"
    );
}

#[gpui_kit::test]
fn next_launch_restores_the_active_tab(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("b"), cx);
    open(win, &ws, info("a"), cx); // re-open focuses `a`
    wait_until(cx, |cx| {
        let s = saved_session(cx);
        s.active.and_then(|i| s.tabs.get(i)) == Some(&info("a"))
    });

    let (_win2, ws2) = open_window(cx, no_project_env());
    assert_eq!(active_spec(&ws2, cx), Some(info("a")));
}

#[gpui_kit::test]
fn next_launch_restores_pinned_tabs(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    ws.update(cx, |ws, cx| {
        let id = ws.active_center_panel_id(cx).expect("active panel");
        ws.toggle_pin_tab(id, cx);
    });
    wait_until(cx, |cx| saved_session(cx).pinned_tabs == [info("a")]);

    let (_win2, ws2) = open_window(cx, no_project_env());
    let pinned = ws2.read_with(cx, |ws, cx| ws.tab_manager().read(cx).pinned_specs());
    assert_eq!(pinned, [info("a")]);
}

#[gpui_kit::test]
fn tabs_for_unavailable_connections_survive_a_save(cx: &mut TestAppContext) {
    // A previous run left a data viewer for a connection that is not (yet) live.
    let gone = TabSpec::DataViewer {
        conn_id: ConnectionId("gone".into()),
        object: "users".into(),
        filter: None,
    };
    let (win, ws) = open_workspace(cx);
    store_session(
        cx,
        &SessionSnapshot {
            tabs: vec![TabSpec::Home, gone.clone()],
            ..Default::default()
        },
    );
    let (win2, ws2) = open_window(cx, no_project_env());
    assert!(
        !specs(&ws2, cx).contains(&gone),
        "cannot open without its connection"
    );

    // Any later change re-saves the session; the pending tab must not be dropped.
    open(win2, &ws2, info("a"), cx);
    wait_saved(cx, &info("a"));
    assert!(
        saved_session(cx).tabs.contains(&gone),
        "pending tab lost on save"
    );
    let _ = (win, ws);
}
