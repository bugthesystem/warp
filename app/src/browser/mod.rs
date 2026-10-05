//! The browser pane: a native web view placed over an area of a pane that WarpUI leaves empty.

#[cfg(not(target_family = "wasm"))]
mod agent;
mod geometry;
mod registry;
mod view;

use warpui::keymap::FixedBinding;
use warpui::{AppContext, View, id};

#[cfg(not(target_family = "wasm"))]
pub use agent::{BrowserAgent, claude_code_setup_command, mcp_token, mcp_url};
pub use registry::{BrowserViewRegistry, sync_webviews};
pub use view::{BrowserView, BrowserViewAction, BrowserViewEvent};

pub fn init(app: &mut AppContext) {
    let context = id!(BrowserView::ui_name());
    app.register_fixed_bindings([
        FixedBinding::new("cmdorctrl-t", BrowserViewAction::NewTab, context.clone()),
        FixedBinding::new(
            "cmdorctrl-w",
            BrowserViewAction::CloseActiveTab,
            context.clone(),
        ),
        FixedBinding::new(
            "cmdorctrl-l",
            BrowserViewAction::FocusUrlField,
            context.clone(),
        ),
        FixedBinding::new("cmdorctrl-r", BrowserViewAction::Reload, context.clone()),
        FixedBinding::new("cmdorctrl-[", BrowserViewAction::GoBack, context.clone()),
        FixedBinding::new("cmdorctrl-]", BrowserViewAction::GoForward, context),
    ]);
}
