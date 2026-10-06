//! Pages shown through a headless Chromium that Warp starts and streams over the DevTools
//! protocol. Warp uses a Chromium-based browser already on the machine, or one it downloaded.

#[cfg(not(target_family = "wasm"))]
mod cdp;
#[cfg(not(target_family = "wasm"))]
mod input;
#[cfg(not(target_family = "wasm"))]
mod install;
#[cfg(not(target_family = "wasm"))]
mod screencast;

#[cfg(not(target_family = "wasm"))]
pub use install::{
    ChromiumDownload, InstallError, install_chromium, installed_chromium,
    parse_known_good_versions, platform_name,
};
#[cfg(not(target_family = "wasm"))]
pub(crate) use screencast::start_screencast;

/// Ends the headless Chromium processes and removes the profiles that page previews of crashed
/// Warp runs left behind. Blocks briefly, so call it off the main thread.
pub fn clean_up_after_crashed_runs() {
    #[cfg(not(target_family = "wasm"))]
    screencast::clean_up_after_crashed_runs();
}

/// Chromium can't be installed from a web build.
#[cfg(target_family = "wasm")]
#[derive(Debug, thiserror::Error)]
#[error("Chromium can't be downloaded here")]
pub struct InstallError;

#[cfg(target_family = "wasm")]
pub fn install_chromium(_dir: &Path) -> Result<PathBuf, InstallError> {
    Err(InstallError)
}

#[cfg(target_family = "wasm")]
fn installed_chromium(_dir: &Path) -> Option<PathBuf> {
    None
}

use std::path::{Path, PathBuf};

/// Overrides which browser previews run, for development and tests.
pub const CHROMIUM_ENV: &str = "WARP_PREVIEW_CHROMIUM";

/// Finds a Chromium-based browser to run previews in: [`CHROMIUM_ENV`], then a copy Warp
/// downloaded into `install_dir`, then a browser installed on the machine.
pub fn find_chromium(install_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(CHROMIUM_ENV).map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    if let Some(path) = install_dir.and_then(installed_chromium) {
        return Some(path);
    }
    system_chromium_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

/// Where browsers are usually installed on this platform, most preferred first.
fn system_chromium_candidates() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        let apps = [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
        ];
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        roots
            .iter()
            .flat_map(|root| apps.iter().map(move |app| root.join(app)))
            .collect()
    } else if cfg!(windows) {
        let suffixes = [
            r"Google\Chrome\Application\chrome.exe",
            r"Microsoft\Edge\Application\msedge.exe",
            r"Chromium\Application\chrome.exe",
        ];
        ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(std::env::var_os)
            .flat_map(|root| {
                suffixes
                    .iter()
                    .map(move |suffix| PathBuf::from(&root).join(suffix))
            })
            .collect()
    } else {
        let names = [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "microsoft-edge",
            "brave-browser",
        ];
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .collect()
    }
}
