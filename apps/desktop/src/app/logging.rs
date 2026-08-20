//! Process logging: `tracing-subscriber` over stderr and a rotating file.
//!
//! Call sites keep using the `log` crate; `tracing-subscriber`'s default
//! `tracing-log` feature forwards them. `RUST_LOG` overrides the default filter.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const LOG_FILE_PREFIX: &str = "based";
const LOG_FILE_SUFFIX: &str = "log";
const MAX_LOG_FILES: usize = 14;
const DEFAULT_FILTER: &str = "warn,based=info,based_quit=warn,based_updater=info";

static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Directory that holds rotating log files.
pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs/Based")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Based")
            .join("logs")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("based")
            .join("logs")
    }
}

/// Install stderr + file subscribers. Safe to call once from `main`.
pub fn init() {
    let dir = log_dir();
    if let Err(err) = fs::create_dir_all(&dir) {
        eprintln!("Could not create log directory {}: {err}", dir.display());
    }

    let file_layer = match RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(&dir)
    {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = FILE_GUARD.set(guard);
            Some(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_target(true)
                    .with_writer(writer),
            )
        }
        Err(err) => {
            eprintln!(
                "Could not open log file in {}: {err}... Defaulting to stderr",
                dir.display()
            );
            None
        }
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(file_layer)
        .init();

    tracing::info!(target: "based", path = %dir.display(), "logging initialized");
}

/// Reveal the log directory in the platform file manager (Help → Open Logs).
pub fn open_logs() {
    let dir = log_dir();
    if let Err(err) = fs::create_dir_all(&dir) {
        tracing::warn!(target: "based", "create log dir: {err:#}");
        return;
    }
    if let Err(err) = open_dir(&dir) {
        tracing::warn!(target: "based", "open logs: {err:#}");
    }
}

fn open_dir(dir: &std::path::Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        drop(process::Command::new("open").arg(dir).spawn()?);
    }
    #[cfg(target_os = "linux")]
    {
        drop(process::Command::new("xdg-open").arg(dir).spawn()?);
    }
    #[cfg(target_os = "windows")]
    {
        let dir_str = dir.display().to_string();
        drop(
            process::Command::new("explorer.exe")
                .arg(&dir_str)
                .spawn()?,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_dir_ends_with_based_logs_folder() {
        let dir = log_dir();
        #[cfg(target_os = "macos")]
        assert!(
            dir.ends_with("Library/Logs/Based"),
            "unexpected macOS log dir: {}",
            dir.display()
        );
        #[cfg(target_os = "windows")]
        assert!(
            dir.ends_with("Based\\logs") || dir.ends_with("Based/logs"),
            "unexpected Windows log dir: {}",
            dir.display()
        );
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(
            dir.ends_with("based/logs"),
            "unexpected Linux log dir: {}",
            dir.display()
        );
    }
}
