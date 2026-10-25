use anyhow::{Context as _, Result};
use cargo_packager_updater::Update;
use gpui_kit::App;

use super::log::info as uinfo;
use super::persist::UpdaterStateFile;
use std::env;
use std::path::PathBuf;
use std::process;

/// Install an already downloaded, signature-verified bundle, then relaunch the app.
pub fn install_and_relaunch(
    update: Update,
    bytes: Vec<u8>,
    pending_notes_version: &str,
    state_path: PathBuf,
) -> Result<()> {
    uinfo(format!("install: installing version={}", update.version));
    update.install(bytes).context("install update")?;

    uinfo(format!(
        "install: success, pending release notes v{pending_notes_version}"
    ));
    let mut state = UpdaterStateFile::load_from(state_path);
    state.set_pending_release_notes(pending_notes_version);
    state.save_best_effort();

    relaunch_app()?;
    uinfo("install: exiting for relaunch");
    process::exit(0);
}

pub fn relaunch_app() -> Result<()> {
    let exe = env::current_exe().context("current_exe")?;
    uinfo(format!("relaunch: spawning {}", exe.display()));
    process::Command::new(exe)
        .args(env::args().skip(1))
        .spawn()
        .context("relaunch spawn")?;
    Ok(())
}

pub fn open_releases_page(cx: &mut App) {
    uinfo("open releases page (manual update fallback)");
    cx.open_url(super::config::RELEASES_PAGE);
}
