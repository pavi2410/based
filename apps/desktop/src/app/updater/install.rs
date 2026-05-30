use anyhow::{Context as _, Result};
use cargo_packager_updater::Update;
use gpui::App;

use super::config::{current_version_string, supports_in_app_install};
use super::log::info as uinfo;
use super::persist::UpdaterStateFile;

/// Download and install a signed update bundle, then relaunch the app.
pub fn download_install_and_relaunch(update: Update, pending_notes_version: &str) -> Result<()> {
    uinfo(format!(
        "install: download_and_install version={} url={}",
        update.version, update.download_url
    ));
    update
        .download_and_install()
        .context("download_and_install update")?;

    uinfo(format!(
        "install: success, pending release notes v{pending_notes_version}"
    ));
    let mut state = UpdaterStateFile::load();
    state.set_pending_release_notes(pending_notes_version);
    state.save_best_effort();

    relaunch_app()?;
    uinfo("install: exiting for relaunch");
    std::process::exit(0);
}

pub fn relaunch_app() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    uinfo(format!("relaunch: spawning {}", exe.display()));
    std::process::Command::new(exe)
        .args(std::env::args().skip(1))
        .spawn()
        .context("relaunch spawn")?;
    Ok(())
}

pub fn open_releases_page(cx: &mut App) {
    uinfo("open releases page (manual update fallback)");
    cx.open_url(super::config::RELEASES_PAGE);
}

pub fn install_mode_label() -> &'static str {
    if supports_in_app_install() {
        "in_app"
    } else {
        "manual"
    }
}

pub fn running_version() -> &'static str {
    current_version_string()
}
