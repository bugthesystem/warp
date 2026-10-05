//! The browser pane: a native web view placed over an area of a pane that WarpUI leaves empty.

mod geometry;
mod registry;
mod view;

pub use registry::{BrowserViewRegistry, sync_webviews};
pub use view::{BrowserView, BrowserViewEvent};
