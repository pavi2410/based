//! VS Code–style tray notifications via `WindowExt::push_notification` (see Phase 7 UI feedback doc).

use gpui::{App, IntoElement, SharedString, prelude::*, px};
use gpui_component::{
    Sizable as _, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    notification::Notification,
};

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

/// Show a success toast after a file export.
///
/// Two explicit buttons are shown inside the notification body:
/// - "Open file" — opens the file with the system default handler
/// - "Show in Finder" / "Show in Explorer" / "Open Folder" — reveals it in the file manager
///
/// The notification body itself is not clickable; all actions are opt-in.
pub fn push_export_success(app: &mut App, path: &std::path::Path) {
    let file_name: SharedString = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
        .into();

    let path_for_open = path.to_path_buf();
    let path_for_reveal = path.to_path_buf();

    #[cfg(target_os = "macos")]
    let reveal_label: SharedString = "Show in Finder".into();
    #[cfg(target_os = "windows")]
    let reveal_label: SharedString = "Show in Explorer".into();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let reveal_label: SharedString = "Open Folder".into();

    let note = Notification::success(file_name)
        .title("Export complete")
        .autohide(false)
        .content(move |_, _, _| {
            let p_open = path_for_open.clone();
            let p_reveal = path_for_reveal.clone();
            let label = reveal_label.clone();
            h_flex()
                .gap(px(4.0))
                .pt(px(4.0))
                .child(
                    Button::new("export-open-file")
                        .ghost()
                        .small()
                        .label("Open file")
                        .on_click(move |_, _, _| {
                            open_path(&p_open);
                        }),
                )
                .child(
                    Button::new("export-reveal")
                        .ghost()
                        .small()
                        .label(label)
                        .on_click(move |_, _, _| {
                            reveal_path(&p_reveal);
                        }),
                )
                .into_any_element()
        });

    push_notification(app, note);
}

/// Open a file with the system default handler.
fn open_path(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    {
        let path_str = path.display().to_string();
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path_str])
            .spawn();
    }
}

/// Reveal a file in the platform file manager (Finder / Files / Explorer).
fn reveal_path(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .args(["-R", &path.display().to_string()])
        .spawn();
    #[cfg(target_os = "linux")]
    {
        let dir = path.parent().unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let arg = format!("/select,{}", path.display());
        let _ = std::process::Command::new("explorer.exe").arg(&arg).spawn();
    }
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
