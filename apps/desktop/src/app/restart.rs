//! Relaunch the running executable (dev tools and updater).

use super::updater::relaunch_app;
use std::process;

/// Spawn a new process with the same argv, then exit the current one.
pub fn restart_app() {
    if let Err(err) = relaunch_app() {
        log::warn!("restart app: {err:#}");
        return;
    }
    process::exit(0);
}
