use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Lists the current Chrome for Testing builds and where to download them.
const KNOWN_GOOD_VERSIONS_URL: &str = "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

/// The Chrome for Testing build Warp downloads: the headless shell, which is small and only
/// renders pages.
const PRODUCT: &str = "chrome-headless-shell";

/// Names the installed version inside the install directory.
const VERSION_FILE: &str = "version.txt";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Chromium can't be downloaded for this platform")]
    UnsupportedPlatform,
    #[error("Couldn't download Chromium: {0}")]
    Download(#[from] reqwest::Error),
    #[error("Couldn't read the list of Chromium builds")]
    BadVersionList,
    #[error("Couldn't unpack Chromium: {0}")]
    Unpack(#[from] zip::result::ZipError),
    #[error("Couldn't install Chromium: {0}")]
    Io(#[from] io::Error),
}

/// A Chrome for Testing build to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromiumDownload {
    pub version: String,
    pub url: String,
}

/// The Chrome for Testing name of this platform, or `None` where there are no builds.
pub fn platform_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("mac-arm64"),
        ("macos", "x86_64") => Some("mac-x64"),
        ("linux", "x86_64") => Some("linux64"),
        ("windows", "x86_64") => Some("win64"),
        ("windows", "x86") => Some("win32"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct KnownGoodVersions {
    channels: Channels,
}

#[derive(Deserialize)]
struct Channels {
    #[serde(rename = "Stable")]
    stable: Channel,
}

#[derive(Deserialize)]
struct Channel {
    version: String,
    downloads: std::collections::HashMap<String, Vec<PlatformDownload>>,
}

#[derive(Deserialize)]
struct PlatformDownload {
    platform: String,
    url: String,
}

/// Reads the stable headless shell build for `platform` out of the Chrome for Testing version
/// list.
pub fn parse_known_good_versions(json: &str, platform: &str) -> Option<ChromiumDownload> {
    let versions: KnownGoodVersions = serde_json::from_str(json).ok()?;
    let stable = versions.channels.stable;
    let url = stable
        .downloads
        .get(PRODUCT)?
        .iter()
        .find(|download| download.platform == platform)?
        .url
        .clone();
    Some(ChromiumDownload {
        version: stable.version,
        url,
    })
}

/// Where the browser binary sits in an unpacked build for `platform`.
fn binary_path(version_dir: &Path, platform: &str) -> PathBuf {
    let binary = if platform.starts_with("win") {
        format!("{PRODUCT}.exe")
    } else {
        PRODUCT.to_owned()
    };
    version_dir
        .join(format!("{PRODUCT}-{platform}"))
        .join(binary)
}

/// The browser Warp downloaded into `dir`, if one is installed there.
pub fn installed_chromium(dir: &Path) -> Option<PathBuf> {
    let version = fs::read_to_string(dir.join(VERSION_FILE)).ok()?;
    let path = binary_path(&dir.join(version.trim()), platform_name()?);
    path.is_file().then_some(path)
}

/// Downloads the current stable headless Chromium into `dir` and returns its binary. Blocks, so
/// call it off the main thread.
pub fn install_chromium(dir: &Path) -> Result<PathBuf, InstallError> {
    let platform = platform_name().ok_or(InstallError::UnsupportedPlatform)?;
    let client = reqwest::blocking::Client::new();
    let list = client
        .get(KNOWN_GOOD_VERSIONS_URL)
        .send()?
        .error_for_status()?
        .text()?;
    let download =
        parse_known_good_versions(&list, platform).ok_or(InstallError::BadVersionList)?;

    fs::create_dir_all(dir)?;
    let mut archive_file = tempfile_in(dir)?;
    client
        .get(&download.url)
        .send()?
        .error_for_status()?
        .copy_to(&mut archive_file.1)?;

    let version_dir = dir.join(&download.version);
    let unpacking = dir.join(format!("{}.partial", download.version));
    let _ = fs::remove_dir_all(&unpacking);
    zip::ZipArchive::new(fs::File::open(&archive_file.0)?)?.extract(&unpacking)?;
    let _ = fs::remove_file(&archive_file.0);
    let _ = fs::remove_dir_all(&version_dir);
    fs::rename(&unpacking, &version_dir)?;

    let binary = binary_path(&version_dir, platform);
    make_executable(&binary)?;
    fs::write(dir.join(VERSION_FILE), &download.version)?;
    Ok(binary)
}

fn tempfile_in(dir: &Path) -> io::Result<(PathBuf, fs::File)> {
    let path = dir.join("download.zip.partial");
    let file = fs::File::create(&path)?;
    Ok((path, file))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // The archive's own modes are not always kept on extraction; the shell's helpers need them
    // too, so mark every file in its directory executable.
    let dir = path.parent().unwrap_or(path);
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let mut permissions = entry.metadata()?.permissions();
            permissions.set_mode(permissions.mode() | 0o755);
            fs::set_permissions(entry.path(), permissions)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
