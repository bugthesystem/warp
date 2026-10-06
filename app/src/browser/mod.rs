//! The browser pane: a native web view placed over an area of a pane that WarpUI leaves empty.

#[cfg(not(target_family = "wasm"))]
mod agent;
mod geometry;
mod history;
#[cfg(not(target_family = "wasm"))]
mod local_servers;
mod registry;
mod view;

use warp_browser::EditCommand;
use warpui::actions::StandardAction;
use warpui::keymap::FixedBinding;
use warpui::{AppContext, View, WindowId, id};

use crate::features::FeatureFlag;

use crate::util::bindings::CustomAction;

#[cfg(not(target_family = "wasm"))]
pub use agent::{BrowserAgent, mcp_token, mcp_url, terminal_env_vars, write_claude_code_plugin};
pub use history::BrowserHistoryModel;
#[cfg(not(target_family = "wasm"))]
pub(crate) use local_servers::detect_local_servers;
pub use registry::{BrowserViewRegistry, sync_webviews};
pub(crate) use view::AgentApproval;
pub use view::{BrowserView, BrowserViewAction, BrowserViewEvent};

/// The Warp window agents' browser and preview tools act in: the active one, or while Warp is not
/// frontmost (an agent in another app's terminal), the one the user used last.
pub(crate) fn agent_window(ctx: &AppContext) -> Option<WindowId> {
    let windows = ctx.windows();
    windows
        .active_window()
        .or_else(|| windows.frontmost_window_id())
        .or_else(|| windows.ordered_window_ids().first().copied())
}

/// Whether browser panes, and the browser tools agents get, are available.
pub fn is_enabled() -> bool {
    FeatureFlag::BrowserPane.is_enabled() && warp_browser::is_supported()
}

/// Whether the local MCP endpoint serves any tools: browser tools, preview tools, or both.
pub fn agent_tools_enabled() -> bool {
    is_enabled() || crate::preview::is_enabled()
}

/// The router serving the local MCP endpoint with the tools that are enabled.
#[cfg(not(target_family = "wasm"))]
pub fn mcp_router(ctx: &AppContext) -> axum::Router {
    use warpui::SingletonEntity as _;

    let channels = warp_browser::agent::ToolChannels {
        browser: is_enabled().then(|| BrowserAgent::as_ref(ctx).requests()),
        preview: crate::preview::is_enabled()
            .then(|| crate::preview::PreviewAgent::as_ref(ctx).requests()),
    };
    warp_browser::agent::router(channels, mcp_token())
}

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
        FixedBinding::new("cmdorctrl-]", BrowserViewAction::GoForward, context.clone()),
    ]);
    // Warp's Edit menu turns these shortcuts into its own actions before the page sees the key
    // events, so the pane passes them on to the web view.
    app.register_fixed_bindings(
        [
            (CustomAction::Cut, EditCommand::Cut, "Cut"),
            (CustomAction::Copy, EditCommand::Copy, "Copy"),
            (CustomAction::Paste, EditCommand::Paste, "Paste"),
            (CustomAction::Undo, EditCommand::Undo, "Undo"),
            (CustomAction::Redo, EditCommand::Redo, "Redo"),
            (
                CustomAction::SelectAll,
                EditCommand::SelectAll,
                "Select All",
            ),
        ]
        .map(|(custom_action, command, description)| {
            FixedBinding::custom(
                custom_action,
                BrowserViewAction::Edit(command),
                description,
                context.clone(),
            )
        }),
    );
    app.register_fixed_bindings([FixedBinding::standard(
        StandardAction::Paste,
        BrowserViewAction::Edit(EditCommand::Paste),
        context,
    )]);
}
