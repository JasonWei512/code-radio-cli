use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::Mutex;
use version_compare::Version;

const LATEST_RELEASE_CACHE_FILE_NAME: &str = "e128c5f5-0a56-41d3-a121-1f2c8bb88417";

static LATEST_RELEASE_CACHE_FILE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut pathbuf = std::env::temp_dir();
    pathbuf.push(LATEST_RELEASE_CACHE_FILE_NAME);
    pathbuf
});

static FILE_IO_MUTEX: Mutex<()> = Mutex::const_new(());

// Use a cache file in temp dir to store latest release info and speed up the process of checking update
pub async fn get_new_release() -> Result<Option<Release>> {
    // Asynchronously fetch latest release info from GitHub, and save it to cache file
    let get_new_release_from_github_task = tokio::spawn(get_new_release_from_github());

    if let Some(cached_latest_release) = try_read_latest_release_from_cache_file().await {
        if release_newer_than_current_package(&cached_latest_release) {
            return Ok(Some(cached_latest_release));
        }
    }
    get_new_release_from_github_task.await?
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
    let _file_io_mutex_guard = FILE_IO_MUTEX.lock().await;

    let cache_file_content = tokio::fs::read_to_string(LATEST_RELEASE_CACHE_FILE_PATH.as_path())
        .await
        .ok()?;
    serde_json::from_str(cache_file_content.as_str()).ok()
}

async fn write_latest_release_to_cache_file(release: &Release) -> Result<()> {
    let _file_io_mutex_guard = FILE_IO_MUTEX.lock().await;

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
            return release_version > current_version;
        }
    }
    false
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: String, // Like "1.3.5"
    pub url: String,
}

// This is for deserializing GitHub's latest release api response
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GithubRelease {
    pub tag_name: String, // Like "v1.3.5"
    pub html_url: String,
}
