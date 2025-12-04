mod connection_id;
mod connection_pool;
mod connectors;
mod error;
mod file_watcher;
mod project_commands;
mod project_db_commands;
mod project_types;
mod variables;

use crate::connection_id::ConnectionRegistry;
use crate::file_watcher::{watch_project_config, unwatch_project_config, FileWatcherState};
use crate::project_commands::{
    delete_query_file, initialize_project, list_query_files, load_env_file_command,
    read_project_config, read_query_file, resolve_connection_config_command,
    write_project_config, write_query_file,
};
use crate::project_db_commands::{
    connect_project_db, get_sqlite_objects, get_mongodb_collections, get_postgres_schemas, 
    get_postgres_tables, close_project_connections, get_connection_info,
    query_sqlite_table, query_postgres_table, query_mongodb_collection,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(ConnectionRegistry::new())
        .manage(FileWatcherState::default())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            initialize_project,
            read_project_config,
            write_project_config,
            load_env_file_command,
            resolve_connection_config_command,
            list_query_files,
            read_query_file,
            write_query_file,
            delete_query_file,
            // File watcher commands
            watch_project_config,
            unwatch_project_config,
            // Project database commands
            connect_project_db,
            get_connection_info,
            get_sqlite_objects,
            get_mongodb_collections,
            get_postgres_schemas,
            get_postgres_tables,
            close_project_connections,
            // Data query commands
            query_sqlite_table,
            query_postgres_table,
            query_mongodb_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
