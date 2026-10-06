//! Browser tools for agents, served over MCP by Warp's local HTTP server.
//!
//! Tool calls arrive on the HTTP server's runtime and are forwarded as [`ToolRequest`]s to the
//! app, which runs them against browser panes on the main thread and replies through the
//! request's channel.

mod command;
mod page;
mod server;
mod token;

pub use command::{BrowserCommand, ToolOutput, ToolRequest};
pub use page::{
    ACTION_DELAY, CONSOLE_CAPTURE_SCRIPT, PagePoint, SCROLL_DURATION, click_now_script,
    console_script, format_console_messages, format_page_snapshot, page_point, point_script,
    prepare_type_script, press_script, read_page_script, scroll_started, scroll_to_script,
    type_now_script,
};
pub use server::{MCP_PATH, router};
pub use token::{load_or_create_token, new_token};
