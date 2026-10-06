//! Live previews of other programs for Warp's preview pane.
//!
//! A preview is a stream of frames from a [`Source`]: a page in a headless Chromium that Warp
//! starts, or another app's window on macOS. Streams run on their own threads and report through
//! an [`async_channel`] of [`StreamEvent`]s; frames are JPEG so the UI can hand them straight to
//! its image cache. Nothing here depends on WarpUI.

pub mod agent;
pub mod browser;
pub mod geometry;
pub mod input;
#[cfg(not(target_family = "wasm"))]
pub mod jpeg;
mod stream;
pub mod window;

use std::sync::Arc;

pub use stream::{Environment, Stream, start};

/// What a preview shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// A page loaded in a headless Chromium owned by the stream.
    Browser { url: String },
    /// Another app's window, captured without bringing it forward. macOS only.
    Window(WindowSource),
}

/// A window of another app, as listed by [`window::list_windows`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowSource {
    /// The platform window id (a `CGWindowID` on macOS).
    pub window_id: u32,
    pub pid: i32,
    pub app_name: String,
    /// The app's bundle identifier on macOS, which approvals are keyed by. Falls back to the app
    /// name where there is none.
    pub app_id: String,
    pub title: String,
}

impl Source {
    /// A short name for cards and tabs: the page URL, or the app and window title.
    pub fn label(&self) -> String {
        match self {
            Source::Browser { url } => url.clone(),
            Source::Window(window) if window.title.is_empty() => window.app_name.clone(),
            Source::Window(window) => format!("{} · {}", window.app_name, window.title),
        }
    }
}

/// One picture from a stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub jpeg: Arc<[u8]>,
    /// Size in pixels.
    pub width: u32,
    pub height: u32,
}

/// How often a stream produces frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rate {
    /// The source's own pace, for the preview in front.
    #[default]
    Full,
    /// A few small frames a second, for cards behind the front one.
    Thumbnail,
    /// No frames, for previews that are not shown anywhere.
    Paused,
}

/// A change reported by a running [`Stream`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    Frame(Frame),
    /// The page's title or the window's title changed.
    Title(String),
    /// The page navigated.
    Url(String),
    /// A console message, uncaught error or log line from the source, ready to show.
    Log(String),
    /// The window is minimized, so no frames arrive until it is restored.
    Minimized(bool),
    /// The stream stopped for good, with the reason to show.
    Ended(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Previewing this is not supported on this platform")]
    Unsupported,
    #[error("Warp needs the Screen Recording permission to show other apps' windows")]
    ScreenRecordingDenied,
    #[error("No Chromium-based browser was found. Download one from the preview pane's start page")]
    NoChromium,
    #[error("The window is no longer open")]
    WindowGone,
    #[error("{0}")]
    Other(String),
}
