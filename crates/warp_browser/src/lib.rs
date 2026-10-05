//! Embeds native web views as child views of Warp windows.
//!
//! The web view is a native OS view layered on top of the window's GPU surface rather than a
//! WarpUI element, so callers position it explicitly with [`WebView::set_bounds`]. Only macOS is
//! supported; on other platforms [`window_parent`] returns `None` and no web view can be created.

#[cfg(not(target_family = "wasm"))]
pub mod agent;
mod url_input;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use mac::{WebView, WebViewParent, window_parent};
#[cfg(not(target_os = "macos"))]
pub use unsupported::{WebView, WebViewParent, window_parent};
pub use url_input::resolve_input;

/// Whether web views can be embedded on the current platform.
pub const fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// A change reported by a [`WebView`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewEvent {
    TitleChanged(String),
    LoadStarted { url: String },
    LoadFinished { url: String },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(target_os = "macos")]
    #[error(transparent)]
    WebView(#[from] wry::Error),
}
