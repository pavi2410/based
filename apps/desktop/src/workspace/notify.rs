//! VS Code–style tray notifications via `WindowExt::push_notification` (see Phase 7 UI feedback doc).

use gpui_kit::component::{
    Sizable as _, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    notification::Notification,
};
use gpui_kit::{App, Global, IntoElement, SharedString, prelude::*, px};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process;

use based_project::SkippedQuery;

use crate::project::loader::SkippedConnection;

/// Push a notification on the active window’s `Root`, or on the first open window
/// while Based is in the background. Returns whether a window took it.
pub fn push_notification(app: &mut App, note: Notification) -> bool {
    let Some(handle) = app
        .active_window()
        .or_else(|| app.windows().into_iter().next())
    else {
        return false;
    };
    handle
        .update(app, |_root, window, app| {
            window.push_notification(note, app);
        })
        .is_ok()
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
            "Based {version} is available — click the version in the status bar."
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
pub fn push_export_success(app: &mut App, path: &Path) {
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

/// Last skipped-connections notice per based-dir, so a reload that changes
/// nothing does not repeat it.
#[derive(Default)]
struct SkippedNoticeSeen(HashMap<PathBuf, Vec<SkippedConnection>>);

impl Global for SkippedNoticeSeen {}

/// A notice for connection files in `based_dir` that did not load, or `None` when
/// there is nothing new to say (none skipped, or the same set as last shown).
/// Call [`remember_skipped_notice`] once it is on screen.
pub fn skipped_connections_notice(
    app: &mut App,
    based_dir: &Path,
    skipped: &[SkippedConnection],
) -> Option<Notification> {
    let seen = &mut app.default_global::<SkippedNoticeSeen>().0;
    if seen.get(based_dir).map(Vec::as_slice).unwrap_or_default() == skipped {
        return None;
    }
    if skipped.is_empty() {
        seen.remove(based_dir);
        return None;
    }

    let lines: Vec<String> = skipped
        .iter()
        .map(|s| match &s.missing_var {
            Some(var) => format!("• {} — needs {var}", s.id),
            None => format!("• {} — {}", s.id, error_one_liner(&s.reason)),
        })
        .collect();
    let needs_secret = skipped.iter().any(|s| s.missing_var.is_some());
    let env_path = based_dir.join(".env");
    let mut message = lines.join("\n");
    if needs_secret {
        message.push_str(&format!(
            "\n\nAdd the missing values to {}.",
            env_path.display()
        ));
    }
    let title = match skipped.len() {
        1 => "1 connection didn't load".to_string(),
        n => format!("{n} connections didn't load"),
    };
    let mut note = Notification::warning(message).title(title).autohide(false);
    if needs_secret {
        note = note.content(move |_, _, _| {
            let env_path = env_path.clone();
            h_flex()
                .pt(px(4.0))
                .child(
                    Button::new("skipped-open-env")
                        .ghost()
                        .small()
                        .label("Open .env")
                        .on_click(move |_, _, _| open_env_file(&env_path)),
                )
                .into_any_element()
        });
    }
    Some(note)
}

/// Record that the notice for this set was shown, so an unchanged reload stays quiet.
pub fn remember_skipped_notice(app: &mut App, based_dir: &Path, skipped: &[SkippedConnection]) {
    app.default_global::<SkippedNoticeSeen>()
        .0
        .insert(based_dir.to_path_buf(), skipped.to_vec());
}

/// Last skipped-queries notice per based-dir (same dedupe as connections).
#[derive(Default)]
struct SkippedQueriesSeen(HashMap<PathBuf, Vec<SkippedQuery>>);

impl Global for SkippedQueriesSeen {}

/// A notice for saved-query files in `based_dir` that did not parse, or `None`
/// when there is nothing new to say. Show it, then call [`remember_skipped_queries`].
pub fn skipped_queries_notice(
    app: &mut App,
    based_dir: &Path,
    skipped: &[SkippedQuery],
) -> Option<Notification> {
    let seen = &mut app.default_global::<SkippedQueriesSeen>().0;
    if seen.get(based_dir).map(Vec::as_slice).unwrap_or_default() == skipped {
        return None;
    }
    if skipped.is_empty() {
        seen.remove(based_dir);
        return None;
    }
    let message = skipped
        .iter()
        .map(|s| format!("• {} — {}", s.path, error_one_liner(&s.reason)))
        .collect::<Vec<_>>()
        .join("\n");
    let title = match skipped.len() {
        1 => "1 saved query didn't load".to_string(),
        n => format!("{n} saved queries didn't load"),
    };
    Some(Notification::warning(message).title(title).autohide(false))
}

/// Record that the notice for this set was shown.
pub fn remember_skipped_queries(app: &mut App, based_dir: &Path, skipped: &[SkippedQuery]) {
    app.default_global::<SkippedQueriesSeen>()
        .0
        .insert(based_dir.to_path_buf(), skipped.to_vec());
}

/// Open the based-dir `.env` in the system editor, creating it empty if missing.
fn open_env_file(path: &Path) {
    if let Err(err) = OpenOptions::new().create(true).append(true).open(path) {
        log::warn!("could not create {}: {err}", path.display());
        return;
    }
    open_path(path);
}

/// Open a file with the system default handler.
fn open_path(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    {
        let path_str = path.display().to_string();
        let _ = process::Command::new("cmd")
            .args(["/C", "start", "", &path_str])
            .spawn();
    }
}

/// Reveal a file in the platform file manager (Finder / Files / Explorer).
fn reveal_path(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = process::Command::new("open")
        .args(["-R", &path.display().to_string()])
        .spawn();
    #[cfg(target_os = "linux")]
    {
        let dir = path.parent().unwrap_or(path);
        let _ = process::Command::new("xdg-open").arg(dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let arg = format!("/select,{}", path.display());
        let _ = process::Command::new("explorer.exe").arg(&arg).spawn();
    }
}

/// First line, capped — for sidebar / inline summaries. Full text lives in tooltips.
pub fn error_one_liner(message: &str) -> SharedString {
    let line = message.lines().next().unwrap_or(message).trim();
    const MAX: usize = 80;
    match line.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &line[..cut]).into(),
        None => line.to_string().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::error_one_liner;

    #[test]
    fn one_liner_cuts_on_char_boundaries() {
        // Byte 80 falls inside the curly quote, which used to panic.
        let quoted = format!("{}\u{201c}tbl\u{201d} does not exist", "x".repeat(79));
        let out = error_one_liner(&quoted);
        assert_eq!(out.chars().count(), 81);
        assert!(out.ends_with('…'));

        let cjk = "関係".repeat(60);
        assert_eq!(error_one_liner(&cjk).chars().count(), 81);
        assert_eq!(error_one_liner("short\nsecond line").as_ref(), "short");
    }
}
