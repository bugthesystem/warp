//! The preview tools agents get beside the browser tools, on the same local MCP endpoint.
//!
//! Previews are for watching in this release, so the tools list what can be shown, open a
//! preview, and look at it. Editing a scene stays with the engine's own MCP server.

use serde_json::{Map, Value, json};

/// Longest side of the picture a tool returns unless the agent asks for full size.
pub const SMALL_PICTURE_SIDE: u32 = 1024;

/// How many log lines `preview_look` returns by default.
pub const DEFAULT_LOG_LINES: usize = 40;

pub const SERVER_INSTRUCTIONS: &str = "Preview tools show other programs live in a Warp preview \
pane: a web page in a headless browser, or another app's window such as a game engine, a game or \
a simulator. Call preview_targets to see what can be shown, preview_open to show it, then \
preview_look to see the picture and recent log in one call. Change scenes and code through the \
engine's own MCP server or the shell, then look again.";

/// A preview tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewCommand {
    Targets,
    Open(OpenTarget),
    Screenshot {
        preview: Option<u64>,
        full_size: bool,
    },
    Look {
        preview: Option<u64>,
        lines: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenTarget {
    Url(String),
    Window(u32),
}

/// What a preview tool returns: text, and a JPEG picture when there is one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewOutput {
    pub text: String,
    pub jpeg: Option<Vec<u8>>,
}

/// A preview tool call waiting to run on the main thread. The reply channel holds one result.
pub struct PreviewToolRequest {
    pub command: PreviewCommand,
    pub reply: async_channel::Sender<Result<PreviewOutput, String>>,
}

impl PreviewCommand {
    /// Whether `name` is a preview tool.
    pub fn is_preview_tool(name: &str) -> bool {
        name.starts_with("preview_")
    }

    /// What the command does, shown to the user while the agent works.
    pub fn step(&self) -> Option<String> {
        match self {
            Self::Targets => None,
            Self::Open(OpenTarget::Url(url)) => Some(format!("Opening {url} in a preview")),
            Self::Open(OpenTarget::Window(_)) => Some("Opening a window in a preview".to_owned()),
            Self::Screenshot { .. } | Self::Look { .. } => {
                Some("Looking at the preview".to_owned())
            }
        }
    }

    /// Parses an MCP tool call. The error is a message for the agent.
    pub fn from_tool_call(name: &str, args: &Map<String, Value>) -> Result<Self, String> {
        let preview = optional_u64(args, "preview")?;
        match name {
            "preview_targets" => Ok(Self::Targets),
            "preview_open" => {
                let url = optional_string(args, "url")?;
                let window = optional_u64(args, "window")?;
                match (url, window) {
                    (Some(url), None) => Ok(Self::Open(OpenTarget::Url(url))),
                    (None, Some(window)) => u32::try_from(window)
                        .map(|window| Self::Open(OpenTarget::Window(window)))
                        .map_err(|_| "`window` is not a window id".to_owned()),
                    (Some(_), Some(_)) => Err("Pass either `url` or `window`, not both".to_owned()),
                    (None, None) => Err("Pass `url` or `window`".to_owned()),
                }
            }
            "preview_screenshot" => Ok(Self::Screenshot {
                preview,
                full_size: optional_bool(args, "full_size")?.unwrap_or(false),
            }),
            "preview_look" => Ok(Self::Look {
                preview,
                lines: optional_u64(args, "lines")?
                    .map(|lines| lines as usize)
                    .unwrap_or(DEFAULT_LOG_LINES),
            }),
            _ => Err(format!("Unknown tool `{name}`")),
        }
    }
}

/// The preview tools, as name, description and JSON schema properties with required keys.
pub fn tool_definitions() -> Vec<(&'static str, &'static str, Value, &'static [&'static str])> {
    let preview = json!({
        "type": "integer",
        "minimum": 0,
        "description": "Preview id from preview_targets. Defaults to the preview most recently opened or looked at."
    });
    vec![
        (
            "preview_targets",
            "List what can be shown in a preview: open previews with their ids, and other apps' \
             windows (app, title, size, window id). Any URL can also be previewed in a headless \
             browser.",
            json!({}),
            &[],
        ),
        (
            "preview_open",
            "Show a web page or another app's window in a Warp preview pane, next to the \
             terminal. Adds to the open preview pane's stack if there is one. Returns the new \
             preview's id.",
            json!({
                "url": {"type": "string", "description": "A page to load in a headless browser, such as a local dev server."},
                "window": {"type": "integer", "minimum": 0, "description": "A window id from preview_targets."}
            }),
            &[],
        ),
        (
            "preview_screenshot",
            "Return the preview's current picture as a JPEG, small unless full_size is set.",
            json!({
                "preview": preview,
                "full_size": {"type": "boolean", "description": "Return the picture at the size it was captured. Defaults to false."}
            }),
            &[],
        ),
        (
            "preview_look",
            "Return the preview's current picture and its recent log in one call: console \
             messages and errors for pages. Use it after changing something to see the result.",
            json!({
                "preview": preview,
                "lines": {"type": "integer", "minimum": 0, "description": "How many recent log lines to return. Defaults to 40."}
            }),
            &[],
        ),
    ]
}

fn optional_string(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

fn optional_u64(args: &Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("`{key}` must be a non-negative integer")),
    }
}

fn optional_bool(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("`{key}` must be a boolean")),
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
