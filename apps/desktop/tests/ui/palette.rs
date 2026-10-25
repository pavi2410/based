//! Command palette: open/dismiss via real key bindings and run a command.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{open_workspace, settle};

fn visible(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> bool {
    ws.read_with(cx, |ws, cx| ws.command_palette().read(cx).is_visible())
}

fn press(win: AnyWindowHandle, key: &str, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| window.press(key, cx))
        .unwrap();
    settle(cx);
}

fn type_text(win: AnyWindowHandle, text: &str, cx: &mut TestAppContext) {
    cx.update_window(win, |_, window, cx| window.input(text, cx))
        .unwrap();
    settle(cx);
}

#[gpui_kit::test]
fn cmd_k_toggles_and_escape_dismisses(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    assert!(!visible(&ws, cx));
    press(win, "cmd-k", cx);
    assert!(visible(&ws, cx), "cmd-k should open the palette");
    press(win, "escape", cx);
    assert!(!visible(&ws, cx), "escape should dismiss it");
}

#[gpui_kit::test]
fn show_home_command_activates_home(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    let info = TabSpec::ObjectInfo {
        conn_id: ConnectionId("t".into()),
        object_name: "a".into(),
        kind_label: "Trigger".into(),
    };
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.dispatch_open_tab(info, window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
    // Dock's live panel, compared to Home's panel id.
    let home_active = |cx: &mut TestAppContext| {
        ws.read_with(cx, |ws, cx| {
            let tm = ws.tab_manager().read(cx);
            let home = tm.tabs.iter().find(|t| t.spec == TabSpec::Home).unwrap();
            ws.active_center_panel_id(cx) == Some(home.view().entity_id())
        })
    };
    assert!(!home_active(cx));

    press(win, "cmd-k", cx);
    type_text(win, "home", cx);
    press(win, "enter", cx);
    cx.update_window(win, |_, window, cx| window.render_frame(cx))
        .unwrap();
    settle(cx);

    assert!(!visible(&ws, cx), "confirming should dismiss the palette");
    assert!(home_active(cx), "Show Home should activate the Home panel");
}
