pub const GITHUB_OWNER: &str = "pavi2410";
pub const GITHUB_REPO: &str = "based";
pub const RELEASES_PAGE: &str = "https://github.com/pavi2410/based/releases";
pub const UPDATE_MANIFEST_URL: &str =
    "https://github.com/pavi2410/based/releases/latest/download/latest.json";

/// Minisign public key generated via `cargo packager signer generate`.
pub const UPDATER_PUBKEY: &str = include_str!("../../../assets/updater-key.pub");

pub fn is_dev_build() -> bool {
    env!("CARGO_PKG_VERSION") == "0.0.0-dev"
}

pub fn current_version_string() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn current_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// Linux `.deb` installs cannot self-update via packager; AppImage / macOS / Windows can.
pub fn supports_in_app_install() -> bool {
    #[cfg(target_os = "linux")]
    {
        !is_deb_install()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[cfg(target_os = "linux")]
fn is_deb_install() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        let exe = exe.canonicalize().unwrap_or(exe);
        let path = exe.to_string_lossy();
        let is_deb_path = path.starts_with("/usr/")
            || path.starts_with("/opt/")
            || path.contains("/.local/share/based/");
        let is_appimage = exe
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |name| name.contains("AppImage"));
        is_deb_path && !is_appimage
    } else {
        false
    }
}
