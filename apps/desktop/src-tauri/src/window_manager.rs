//! Multi-window foundation.
//!
//! The app spawns additional webview windows for:
//! - Detached workspace tabs (`WindowKind::Tab`)
//! - Pop-out result viewers (`WindowKind::ResultViewer`)
//! - Settings (`WindowKind::Settings`)
//!
//! Every window is a React app rendering the same `index.html`, but with
//! a `kind` URL query param that the frontend router uses to decide which
//! root component to mount. This keeps the build simple (one bundle, one
//! webview config) while giving us proper OS windows with titlebars,
//! dock icons, and drag-to-detach behaviour.
//!
//! The main window always has label `"main"`; every other window gets a
//! deterministic label like `"tab:<id>"` so we can refocus instead of
//! spawning duplicates.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_decorum::WebviewWindowExt;

use crate::address::TabAddress;

/// The kinds of windows the app can spawn. Encoded as a tagged enum so
/// the frontend can pattern-match in its router.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WindowKind {
    /// Detached workspace tab (query editor or table browser).
    Tab { address: TabAddress },
    /// Pop-out for a query result set.
    ResultViewer { title: String },
    /// App settings window.
    Settings,
}

impl WindowKind {
    /// Stable OS window label. Two calls with the same payload resolve to
    /// the same window so we can refocus rather than spawn duplicates.
    pub fn label(&self) -> String {
        match self {
            WindowKind::Tab { address } => match address {
                TabAddress::Query { connection, id } => {
                    format!("tab:query:{}:{}", connection.conn_key, id)
                }
                TabAddress::Table {
                    connection,
                    schema,
                    name,
                } => format!(
                    "tab:table:{}:{}:{}",
                    connection.conn_key,
                    schema.as_deref().unwrap_or(""),
                    name
                ),
                TabAddress::Inspector {
                    connection,
                    schema,
                    name,
                } => format!(
                    "tab:inspector:{}:{}:{}",
                    connection.conn_key,
                    schema.as_deref().unwrap_or(""),
                    name
                ),
            },
            WindowKind::ResultViewer { title } => format!("result:{}", slug(title)),
            WindowKind::Settings => "settings".to_string(),
        }
    }

    /// Human-readable title shown in the OS window titlebar.
    pub fn title(&self) -> String {
        match self {
            WindowKind::Tab { address } => match address {
                TabAddress::Query { id, .. } => format!("Query — {}", id),
                TabAddress::Table { name, .. } => format!("Table — {}", name),
                TabAddress::Inspector { name, .. } => format!("Inspector — {}", name),
            },
            WindowKind::ResultViewer { title } => title.clone(),
            WindowKind::Settings => "Settings".to_string(),
        }
    }

    fn initial_size(&self) -> (f64, f64) {
        match self {
            WindowKind::Settings => (720.0, 540.0),
            _ => (900.0, 640.0),
        }
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Open (or refocus, if already open) a child window.
#[command]
#[specta::specta]
pub async fn open_window(app: AppHandle, kind: WindowKind) -> Result<String, String> {
    let label = kind.label();

    if let Some(existing) = app.get_webview_window(&label) {
        existing
            .set_focus()
            .map_err(|e| format!("Failed to focus window '{}': {}", label, e))?;
        return Ok(label);
    }

    let (w, h) = kind.initial_size();
    let payload = serde_json::to_string(&kind)
        .map_err(|e| format!("Failed to encode window kind: {}", e))?;
    let encoded = urlencoding::encode(&payload).into_owned();
    let url = format!("index.html?window={}", encoded);

    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(kind.title())
        .inner_size(w, h);

    #[cfg(target_os = "macos")]
    {
        use tauri::TitleBarStyle;
        builder = builder.title_bar_style(TitleBarStyle::Overlay).hidden_title(true);
    }

    let window = builder
        .build()
        .map_err(|e| format!("Failed to build window '{}': {}", label, e))?;

    // Match the main window's chrome: overlay titlebar with inset traffic
    // lights on macOS.
    window
        .create_overlay_titlebar()
        .map_err(|e| format!("Failed to create overlay titlebar for '{}': {}", label, e))?;

    #[cfg(target_os = "macos")]
    {
        let _ = window.set_traffic_lights_inset(16.0, 24.0);
    }

    Ok(label)
}

/// Focus the window with the given label. No-op if it does not exist.
#[command]
#[specta::specta]
pub async fn focus_window(app: AppHandle, label: String) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(&label) {
        w.set_focus()
            .map_err(|e| format!("Failed to focus window '{}': {}", label, e))?;
    }
    Ok(())
}

/// Close the window with the given label. No-op if it does not exist.
/// Refuses to close the `"main"` label to avoid killing the app.
#[command]
#[specta::specta]
pub async fn close_window(app: AppHandle, label: String) -> Result<(), String> {
    if label == "main" {
        return Err("Refusing to close main window".to_string());
    }
    if let Some(w) = app.get_webview_window(&label) {
        w.close()
            .map_err(|e| format!("Failed to close window '{}': {}", label, e))?;
    }
    Ok(())
}
