mod commands;
mod db_pool;
mod decode;
mod error;

use crate::commands::{close, load, query};
use crate::db_pool::DbPool;
use crate::error::Error;
use std::collections::HashMap;
use tauri::Manager;
use tokio::sync::RwLock;
use window_vibrancy::*;

#[derive(Default)]
pub struct DbInstances(pub RwLock<HashMap<String, DbPool>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(DbInstances::default())
        .invoke_handler(tauri::generate_handler![
            load,
            close,
            query,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            #[cfg(target_os = "macos")]
            apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None)
                .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");

            #[cfg(target_os = "windows")]
            apply_mica(&window, None)
                .expect("Unsupported platform! 'apply_blur' is only supported on Windows");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
