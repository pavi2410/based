//! Tab strip helpers shared by the UI tests.

use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use super::settle;

/// Open `spec` through the real dispatch path; panics if it is refused.
pub fn open_tab(
    win: AnyWindowHandle,
    ws: &Entity<Workspace>,
    spec: TabSpec,
    cx: &mut TestAppContext,
) {
    cx.update_window(win, |_, window, cx| {
        let opened = ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec.clone(), window, cx));
        assert!(opened, "dispatch_open_tab refused {spec:?}");
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

/// Specs of every tab, in strip order.
pub fn specs(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<TabSpec> {
    ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .map(|t| t.spec.clone())
            .collect()
    })
}

pub fn active_spec(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Option<TabSpec> {
    ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .active_tab()
            .map(|t| t.spec.clone())
    })
}
