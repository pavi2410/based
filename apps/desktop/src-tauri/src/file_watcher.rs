use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::channel;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

pub struct FileWatcherState {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
}

impl FileWatcherState {
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for FileWatcherState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start watching the project config file and env file for changes
#[tauri::command]
#[specta::specta]
pub async fn watch_project_config(
    app_handle: AppHandle,
    project_path: String,
    state: tauri::State<'_, FileWatcherState>,
) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".based/config.toml");
    let env_path = Path::new(&project_path).join(".based/.env");

    // Stop existing watcher if any
    let mut watcher_guard = state.watcher.lock().await;
    *watcher_guard = None;
    drop(watcher_guard);

    // Create channel for receiving file system events
    let (tx, rx) = channel();

    // Create watcher
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .map_err(|e| format!("Failed to create watcher: {}", e))?;

    // Watch the config file
    watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch config file: {}", e))?;

    // Watch the env file (if it exists)
    if env_path.exists() {
        watcher
            .watch(&env_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch env file: {}", e))?;
    }

    // Store the watcher
    let mut watcher_guard = state.watcher.lock().await;
    *watcher_guard = Some(watcher);
    drop(watcher_guard);

    // Spawn task to handle events with debouncing
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        use std::time::Duration;
        let mut last_emit = std::time::Instant::now();
        let debounce_duration = Duration::from_millis(500);

        loop {
            match rx.recv() {
                Ok(result) => match result {
                    Ok(event) => {
                        if should_reload_config(&event) {
                            let now = std::time::Instant::now();
                            // Only emit if enough time has passed since last emit
                            if now.duration_since(last_emit) >= debounce_duration {
                                let _ = app_handle_clone.emit("config-changed", ());
                                last_emit = now;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Watch error: {}", e);
                    }
                },
                Err(_) => {
                    // Channel closed, exit loop
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Stop watching the project config file
#[tauri::command]
#[specta::specta]
pub async fn unwatch_project_config(
    state: tauri::State<'_, FileWatcherState>,
) -> Result<(), String> {
    let mut watcher_guard = state.watcher.lock().await;
    *watcher_guard = None;
    Ok(())
}

fn should_reload_config(event: &Event) -> bool {
    use notify::EventKind;

    match event.kind {
        EventKind::Modify(_) | EventKind::Create(_) => true,
        _ => false,
    }
}
