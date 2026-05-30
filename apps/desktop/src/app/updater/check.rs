use anyhow::{Context as _, Result};
use cargo_packager_updater::{Config, Update, WindowsConfig, WindowsUpdateInstallMode};
use semver::Version;
use url::Url;

use super::config::{
    UPDATE_MANIFEST_URL, UPDATER_PUBKEY, current_version, supports_in_app_install,
};

pub fn updater_config() -> Result<Config> {
    Ok(Config {
        endpoints: vec![Url::parse(UPDATE_MANIFEST_URL).context("update manifest url")?],
        pubkey: UPDATER_PUBKEY.to_string(),
        windows: Some(WindowsConfig {
            install_mode: Some(WindowsUpdateInstallMode::Passive),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// Check packager manifest for a signed update newer than the running version.
pub fn check_packager_update() -> Result<Option<Update>> {
    if !supports_in_app_install() {
        return Ok(None);
    }
    let config = updater_config()?;
    let current = current_version();
    cargo_packager_updater::check_update(current, config).context("packager check_update")
}

pub fn is_newer(candidate: &Version, current: &Version) -> bool {
    candidate > current
}
