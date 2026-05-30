//! VS Code–style tray notifications via `WindowExt::push_notification` (see Phase 7 UI feedback doc).

use gpui::{App, SharedString};
use gpui_component::{WindowExt, notification::Notification};

/// Push a notification on the **active** window’s `Root`. No-op if there is no active window.
pub fn push_notification(app: &mut App, note: Notification) {
    let Some(handle) = app.active_window() else {
        return;
    };
    let _ = handle.update(app, |_root, window, app| {
        window.push_notification(note, app);
    });
}

pub fn push_error(app: &mut App, title: impl Into<SharedString>, message: impl Into<SharedString>) {
    push_notification(app, Notification::error(message).title(title));
}

/// Critical: database unreachable, auth failure, etc. Stays until the user dismisses it.
pub fn push_connection_failure(
    app: &mut App,
    conn_label: impl Into<SharedString>,
    engine_short: impl Into<SharedString>,
    err_detail: impl Into<SharedString>,
) {
    let conn_label = conn_label.into();
    let engine = engine_short.into();
    let detail = err_detail.into();
    let message: SharedString = if detail.is_empty() {
        format!("{conn_label} ({engine})").into()
    } else {
        format!("{conn_label} ({engine})\n\n{detail}").into()
    };
    push_notification(
        app,
        Notification::error(message)
            .title("Couldn't connect")
            .autohide(false),
    );
}

pub fn push_info(app: &mut App, message: impl Into<SharedString>) {
    push_notification(app, Notification::info(message));
}

pub fn push_update_available(app: &mut App, version: &str) {
    push_notification(
        app,
        Notification::info(format!(
            "Based {version} is available — see the status bar for options."
        ))
        .title("Update available"),
    );
}

/// First line, capped — for sidebar / inline summaries. Full text lives in tooltips.
pub fn error_one_liner(message: &str) -> SharedString {
    let line = message.lines().next().unwrap_or(message).trim();
    const MAX: usize = 80;
    if line.len() > MAX {
        format!("{}…", &line[..MAX]).into()
    } else {
        line.to_string().into()
    }
}
