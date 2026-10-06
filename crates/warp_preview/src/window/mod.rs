//! Other apps' windows, captured without bringing them forward. macOS only: elsewhere nothing is
//! listed and capture is unsupported.

#[cfg(target_os = "macos")]
mod input_mac;
#[cfg(target_os = "macos")]
mod mac;

use crate::WindowSource;

/// A window that can be previewed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowEntry {
    pub source: WindowSource,
    /// Size in points.
    pub width: u32,
    pub height: u32,
    /// The window layer; 0 is a normal app window.
    pub layer: i64,
    /// False for minimized windows and windows on another Space.
    pub on_screen: bool,
}

/// Windows smaller than this on either side are palettes, tooltips or status items.
const MIN_SIDE: u32 = 80;

/// Keeps the windows worth offering: normal app windows of other processes that are big enough
/// to watch, on-screen ones first.
pub fn previewable(mut windows: Vec<WindowEntry>, own_pid: i32) -> Vec<WindowEntry> {
    windows.retain(|window| {
        window.layer == 0
            && window.source.pid != own_pid
            && window.width >= MIN_SIDE
            && window.height >= MIN_SIDE
            && !window.source.app_name.is_empty()
    });
    windows.sort_by_key(|window| !window.on_screen);
    windows
}

/// Whether other apps' windows can be previewed on this platform.
pub const fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Whether Warp may capture other apps' windows. Never prompts.
pub fn has_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::has_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Asks the system for the Screen Recording permission, which shows its prompt the first time.
pub fn request_permission() {
    #[cfg(target_os = "macos")]
    mac::request_permission();
}

/// Opens the system settings page where the user grants Screen Recording.
/// Whether Warp may send input to other apps' windows (macOS Accessibility). Never prompts.
pub fn has_input_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::has_input_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Shows the system prompt for the Accessibility permission, when macOS still offers it.
pub fn request_input_permission() {
    #[cfg(target_os = "macos")]
    mac::request_input_permission();
}

/// The settings page where the Accessibility permission is granted.
pub const INPUT_PERMISSION_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

pub const PERMISSION_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

/// The windows of other apps that can be previewed, on-screen ones first. Blocks briefly, so call
/// it off the main thread.
pub fn list_windows() -> Result<Vec<WindowEntry>, crate::Error> {
    #[cfg(target_os = "macos")]
    {
        mac::list_windows().map(|windows| previewable(windows, std::process::id() as i32))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::Error::Unsupported)
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn start_capture(
    source: WindowSource,
    rate: crate::Rate,
    events: async_channel::Sender<crate::StreamEvent>,
) -> Result<crate::Stream, crate::Error> {
    #[cfg(target_os = "macos")]
    {
        mac::start_capture(source, rate, events)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source, rate, events);
        Err(crate::Error::Unsupported)
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
