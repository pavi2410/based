//! Quit / window-close confirmation when live database connections are open.

use gpui::{App, Context, Entity, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};

use crate::connection::registry::ConnectionRegistry;
use crate::connection::{
    ConnectionState, LiveConnection, close_any_connection, live_connection_count, live_connections,
};
use crate::widgets::{engine_icon, engine_label_inline};
use crate::workspace::Workspace;
use crate::workspace::WorkspaceRef;

const LOG_TARGET: &str = "based_quit";

#[derive(Clone, Copy, Debug)]
enum QuitMode {
    CloseWindow,
    QuitApp,
}

fn trace(msg: &str) {
    log::warn!(target: LOG_TARGET, "{msg}");
}

fn trace_connections(registry: &Entity<ConnectionRegistry>, cx: &App, context: &str) {
    let count = live_connection_count(registry, cx);
    trace(&format!("{context}: live_connection_count={count}"));
    for ent in registry.read(cx).connections() {
        let entry = ent.read(cx);
        trace(&format!(
            "  connection id={} label={} state={}",
            entry.id,
            entry.config.label(),
            entry.state.label()
        ));
    }
}

/// Close the window, prompting when live connections are open.
pub fn request_window_close(
    registry: &Entity<ConnectionRegistry>,
    window: &mut Window,
    cx: &mut App,
) {
    trace_connections(registry, cx, "request_window_close");
    if live_connection_count(registry, cx) == 0 {
        trace("request_window_close: no live connections, removing window");
        window.remove_window();
        return;
    }
    if window.has_active_dialog(cx) {
        trace("request_window_close: dialog already active, skipping");
        return;
    }
    trace("request_window_close: opening alert dialog");
    show_quit_dialog(registry.clone(), window, cx, QuitMode::CloseWindow);
}

/// GPUI `on_window_should_close` handler — return `true` to allow close, `false` to block.
pub fn confirm_before_close_window(
    registry: &Entity<ConnectionRegistry>,
    workspace: &Entity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    trace("confirm_before_close_window: platform callback fired");
    trace_connections(registry, cx, "confirm_before_close_window");

    if live_connection_count(registry, cx) == 0 {
        trace("confirm_before_close_window: allow close (no live connections)");
        return true;
    }
    if window.has_active_dialog(cx) {
        trace("confirm_before_close_window: block close (dialog already open)");
        return false;
    }

    trace("confirm_before_close_window: queue dialog for next render");
    queue_close_confirm(workspace, cx);
    false
}

/// Open a queued quit dialog during a normal render frame (reliable vs. platform close callback).
pub fn maybe_show_pending_close_dialog(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if !workspace.pending_close_confirm {
        return;
    }
    workspace.pending_close_confirm = false;
    let registry = workspace.registry().clone();

    trace("maybe_show_pending_close_dialog: showing queued dialog");
    trace_connections(&registry, cx, "maybe_show_pending_close_dialog");

    if live_connection_count(&registry, cx) == 0 {
        trace("maybe_show_pending_close_dialog: no live connections, removing window");
        window.remove_window();
        return;
    }
    if window.has_active_dialog(cx) {
        trace("maybe_show_pending_close_dialog: dialog already active");
        return;
    }
    show_quit_dialog(registry, window, cx, QuitMode::CloseWindow);
}

fn queue_close_confirm(workspace: &Entity<Workspace>, cx: &mut App) {
    workspace.update(cx, |ws, cx| {
        ws.pending_close_confirm = true;
        cx.notify();
        trace("queue_close_confirm: pending_close_confirm=true, notified");
    });
}

/// Menu / ⌘Q quit — shows the same confirmation when connections are live.
pub fn request_app_quit(cx: &mut App) {
    trace("request_app_quit");
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|ws| ws.0.clone()) else {
        trace("request_app_quit: no WorkspaceRef, calling cx.quit()");
        cx.quit();
        return;
    };

    let registry = ws.read(cx).registry().clone();
    trace_connections(&registry, cx, "request_app_quit");
    if live_connection_count(&registry, cx) == 0 {
        trace("request_app_quit: no live connections, quitting");
        cx.quit();
        return;
    }

    let Some(handle) = cx.active_window() else {
        trace("request_app_quit: no active window, disconnect_all + quit");
        disconnect_all(&registry, cx);
        cx.quit();
        return;
    };

    let _ = handle.update(cx, |_, window, cx| {
        if window.has_active_dialog(cx) {
            trace("request_app_quit: dialog already active");
            return;
        }
        trace("request_app_quit: opening alert dialog");
        show_quit_dialog(registry, window, cx, QuitMode::QuitApp);
    });
}

fn show_quit_dialog(
    registry: Entity<ConnectionRegistry>,
    window: &mut Window,
    cx: &mut App,
    mode: QuitMode,
) {
    let live = live_connections(&registry, cx);
    trace(&format!(
        "show_quit_dialog: mode={:?} live={} has_active_dialog={}",
        mode,
        live.len(),
        window.has_active_dialog(cx)
    ));
    let registry_for_ok = registry.clone();

    window.open_alert_dialog(cx, move |alert, _window, cx| {
        trace("show_quit_dialog: open_alert_dialog builder invoked");
        let registry = registry_for_ok.clone();
        let theme = cx.theme();
        let quit_btn = Button::new("quit-confirm")
            .label("Quit")
            .primary()
            .bg(theme.red)
            .border_color(theme.red)
            .text_color(theme.primary_foreground);
        let connection_list = render_live_connection_list(&live, cx);
        alert
            .title("Quit with active connections?")
            .description("Quitting will disconnect these connections:")
            .child(connection_list)
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("quit-cancel").outline().label("Cancel")),
                    )
                    .child(DialogAction::new().child(quit_btn)),
            )
            .on_ok(move |_, window, cx| {
                trace("show_quit_dialog: user confirmed quit");
                disconnect_all(&registry, cx);
                match mode {
                    QuitMode::CloseWindow => window.remove_window(),
                    QuitMode::QuitApp => cx.quit(),
                }
                true
            })
            .on_cancel(|_, _, _| {
                trace("show_quit_dialog: user cancelled");
                true
            })
    });
    trace(&format!(
        "show_quit_dialog: after open_alert_dialog has_active_dialog={}",
        window.has_active_dialog(cx)
    ));
}

fn render_live_connection_list(live: &[LiveConnection], cx: &mut App) -> impl IntoElement {
    let foreground = cx.theme().foreground;
    v_flex()
        .mt_2()
        .gap_1()
        .max_h(px(160.0))
        .overflow_y_scrollbar()
        .children(live.iter().map(|conn| {
            h_flex()
                .gap_2()
                .items_center()
                .child(engine_icon(conn.engine))
                .child(
                    div()
                        .text_sm()
                        .text_color(foreground)
                        .child(conn.label.clone()),
                )
                .child(engine_label_inline(conn.engine, cx))
        }))
}

pub fn disconnect_all(registry: &Entity<ConnectionRegistry>, cx: &mut App) {
    trace("disconnect_all");
    let entries = registry.read(cx).connections().to_vec();
    for ent in entries {
        ent.update(cx, |entry, cx| {
            if let ConnectionState::Connected(ac) =
                std::mem::replace(&mut entry.state, ConnectionState::Disconnected)
            {
                close_any_connection(ac, cx);
            }
            entry.last_error = None;
            cx.notify();
        });
    }
}
