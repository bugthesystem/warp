//! Browser tools for agents, served over MCP by Warp's local HTTP server.
//!
//! Tool calls arrive on the HTTP server's runtime and are forwarded as [`ToolRequest`]s to the
//! app, which runs them against browser panes on the main thread and replies through the
//! request's channel.

mod command;
mod page;
mod server;

pub use command::{BrowserCommand, ToolOutput, ToolRequest};
pub use page::{click_script, format_page_snapshot, read_page_script, type_script};
pub use server::{MCP_PATH, router, session_token};
