//! Tab strip lifecycle: Home respawn, dedup, pinning.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::open_workspace;

fn info(name: &str) -> TabSpec {
    TabSpec::ObjectInfo {
        conn_id: ConnectionId("t".into()),
        object_name: name.into(),
        kind_label: "Trigger".into(),
    }
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

fn open(win: AnyWindowHandle, ws: &Entity<Workspace>, spec: TabSpec, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| {
        let opened = ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec.clone(), window, cx));
        assert!(opened, "dispatch_open_tab refused {spec:?}");
        window.render_frame(cx);
    })
    .unwrap();
    assert!(specs(ws, cx).contains(&spec), "tab missing after open");
}

#[gpui_kit::test]
fn starts_with_home_tab(cx: &mut TestAppContext) {
    let (_win, ws) = open_workspace(cx);
    assert_eq!(specs(&ws, cx), vec![TabSpec::Home]);
}

#[gpui_kit::test]
fn opening_same_spec_twice_does_not_duplicate(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    open(win, &ws, info("a"), cx);
    let tabs = specs(&ws, cx);
    assert_eq!(tabs.iter().filter(|s| **s == info("a")).count(), 1);
}

#[gpui_kit::test]
fn closing_active_tab_removes_it(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    open(win, &ws, info("a"), cx);
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.close_active_center_tab(window, cx));
    })
    .unwrap();
    assert!(!specs(&ws, cx).contains(&info("a")));
}

#[gpui_kit::test]
fn closing_last_tab_respawns_home(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    for _ in 0..2 {
        cx.update_window(win, |_, window, cx| {
            ws.update(cx, |ws, cx| ws.close_active_center_tab(window, cx));
        })
        .unwrap();
    }
    assert_eq!(specs(&ws, cx), vec![TabSpec::Home]);
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
    assert!(specs(&ws, cx).contains(&info("a")));
}
