//! The browser pane: a native web view placed over an area of a pane that WarpUI leaves empty.

#[cfg(not(target_family = "wasm"))]
mod agent;
mod geometry;
mod registry;
mod view;

#[cfg(not(target_family = "wasm"))]
pub use agent::BrowserAgent;
pub use registry::{BrowserViewRegistry, sync_webviews};
pub use view::{BrowserView, BrowserViewEvent};
