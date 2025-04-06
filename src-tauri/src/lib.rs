mod commands;
mod connection_pool;
mod decode;
mod error;

use crate::commands::{close, load, query};
use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct DbInstances(pub RwLock<HashMap<String, ConnectionPool>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(DbInstances::default())
        .invoke_handler(tauri::generate_handler![load, close, query,])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
