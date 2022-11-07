use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use version_compare::Version;

const LATEST_RELEASE_CACHE_FILE_NAME: &str = "e128c5f5-0a56-41d3-a121-1f2c8bb88417";

static LATEST_RELEASE_CACHE_FILE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut pathbuf = std::env::temp_dir();
    pathbuf.push(LATEST_RELEASE_CACHE_FILE_NAME);
    pathbuf
});

pub async fn get_new_release() -> Result<Option<Release>> {
    // Fetch latest release info from GitHub, and save it to cache file in background
    let get_new_release_from_github_task = tokio::spawn(get_new_release_from_github()); 

    if let Some(cached_latest_release) = try_read_latest_release_from_cache_file().await {
        if release_newer_than_current_package(&cached_latest_release) {
            return Ok(Some(cached_latest_release));
        }
    }
    return get_new_release_from_github_task.await?;
}

async fn get_new_release_from_github() -> Result<Option<Release>> {
    let latest_release_from_github = get_latest_release_from_github().await?;
    if release_newer_than_current_package(&latest_release_from_github) {
        Ok(Some(latest_release_from_github))
    } else {
        Ok(None)
    }
}

async fn get_latest_release_from_github() -> Result<Release> {
    let latest_github_response: GithubRelease = reqwest::Client::new()
        .get("https://api.github.com/repos/JasonWei512/code-radio-cli/releases/latest")
        .header(
            "User-Agent",
            "https://github.com/JasonWei512/code-radio-cli",
        )
        .send()
        .await?
        .json()
        .await?;

    let latest_release = Release {
        version: latest_github_response.tag_name.chars().skip(1).collect(),
        url: latest_github_response.html_url.to_owned(),
    };

    let _ = write_latest_release_to_cache_file(&latest_release).await;

    Ok(latest_release)
}

async fn try_read_latest_release_from_cache_file() -> Option<Release> {
    let cache_file_content = tokio::fs::read_to_string(LATEST_RELEASE_CACHE_FILE_PATH.as_path())
        .await
        .ok()?;
    serde_json::from_str(cache_file_content.as_str()).ok()
}

async fn write_latest_release_to_cache_file(release: &Release) -> Result<()> {
    let cache_file_content = serde_json::to_string_pretty(release)?;
    tokio::fs::write(
        &LATEST_RELEASE_CACHE_FILE_PATH.as_path(),
        cache_file_content.as_bytes(),
    )
    .await?;
    Ok(())
}

fn release_newer_than_current_package(release: &Release) -> bool {
    if let Some(current_version) = Version::from(env!("CARGO_PKG_VERSION")) {
        if let Some(release_version) = Version::from(&release.version) {
            if release_version > current_version {
                return true;
            }
        }
    }
    false
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GithubRelease {
    pub tag_name: String,
    pub html_url: String,

    pub draft: bool,
    pub prerelease: bool,
}
