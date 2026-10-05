//! The browser pane: a native web view placed over an area of a pane that WarpUI leaves empty.

#[cfg(not(target_family = "wasm"))]
mod agent;
mod geometry;
mod registry;
mod view;

use warpui::keymap::FixedBinding;
use warpui::{AppContext, View, id};

#[cfg(not(target_family = "wasm"))]
pub use agent::BrowserAgent;
pub use registry::{BrowserViewRegistry, sync_webviews};
pub use view::{BrowserView, BrowserViewAction, BrowserViewEvent};

pub fn init(app: &mut AppContext) {
    app.register_fixed_bindings([FixedBinding::new(
        "cmdorctrl-t",
        BrowserViewAction::NewTab,
        id!(BrowserView::ui_name()),
    )]);
}
