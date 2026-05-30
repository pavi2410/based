use anyhow::{Context as _, Result};
use serde::Deserialize;

use super::config::{GITHUB_OWNER, GITHUB_REPO};
use super::log::{debug as udebug, info as uinfo, warn as uwarn};

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
    prerelease: bool,
}

fn tag_to_version(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

/// Fetch release notes markdown for a CalVer version (with or without leading `v`).
pub async fn fetch_release_body(version: &str) -> Result<String> {
    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    let url =
        format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/tags/{tag}");
    udebug(format!("fetch_release_body: GET {url}"));
    let client = reqwest::Client::builder()
        .user_agent("based-desktop")
        .build()?;
    let release: GitHubRelease = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("github release request")?
        .error_for_status()
        .context("github release status")?
        .json()
        .await
        .context("github release json")?;
    Ok(release.body)
}

#[derive(Debug, Clone)]
pub struct LatestReleaseInfo {
    pub version: semver::Version,
    pub version_label: String,
    pub prerelease: bool,
}

/// Query `/releases/latest` for version discovery (complements packager manifest check).
pub async fn fetch_latest_release(include_prereleases: bool) -> Result<Option<LatestReleaseInfo>> {
    let url = format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest");
    udebug(format!(
        "fetch_latest_release: GET {url} include_prereleases={include_prereleases}"
    ));
    let client = reqwest::Client::builder()
        .user_agent("based-desktop")
        .build()?;
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("github latest release request")?;
    if !response.status().is_success() {
        uwarn(format!("fetch_latest_release: HTTP {}", response.status()));
        anyhow::bail!("github latest release: {}", response.status());
    }
    let release: GitHubRelease = response.json().await.context("github latest json")?;
    if release.prerelease && !include_prereleases {
        udebug(format!(
            "fetch_latest_release: prerelease {} filtered",
            release.tag_name
        ));
        return Ok(None);
    }
    let version_label = tag_to_version(&release.tag_name);
    uinfo(format!("fetch_latest_release: latest={version_label}"));
    let version = semver::Version::parse(&version_label)
        .with_context(|| format!("invalid release tag version {version_label}"))?;
    Ok(Some(LatestReleaseInfo {
        version,
        version_label,
        prerelease: release.prerelease,
    }))
}
