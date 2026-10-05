use serde_json::{Map, Value};

/// An action an agent asked a browser pane to take. `tab` selects a browser pane by the id
/// `browser_tabs` reports; `None` means the most recently used one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserCommand {
    Open {
        url: String,
    },
    ListTabs,
    Navigate {
        tab: Option<u64>,
        url: String,
    },
    Read {
        tab: Option<u64>,
    },
    Screenshot {
        tab: Option<u64>,
    },
    Click {
        tab: Option<u64>,
        element: u64,
    },
    Type {
        tab: Option<u64>,
        element: u64,
        text: String,
        submit: bool,
    },
}

/// What a tool returns to the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolOutput {
    Text(String),
    Png(Vec<u8>),
}

/// A tool call waiting to run on the main thread.
pub struct ToolRequest {
    pub command: BrowserCommand,
    pub reply: tokio::sync::oneshot::Sender<Result<ToolOutput, String>>,
}

impl BrowserCommand {
    /// Parses an MCP tool call. The error is a message for the agent.
    pub fn from_tool_call(name: &str, args: &Map<String, Value>) -> Result<Self, String> {
        let tab = optional_u64(args, "tab")?;
        match name {
            "browser_open" => Ok(Self::Open {
                url: required_string(args, "url")?,
            }),
            "browser_tabs" => Ok(Self::ListTabs),
            "browser_navigate" => Ok(Self::Navigate {
                tab,
                url: required_string(args, "url")?,
            }),
            "browser_read" => Ok(Self::Read { tab }),
            "browser_screenshot" => Ok(Self::Screenshot { tab }),
            "browser_click" => Ok(Self::Click {
                tab,
                element: required_u64(args, "element")?,
            }),
            "browser_type" => Ok(Self::Type {
                tab,
                element: required_u64(args, "element")?,
                text: required_string(args, "text")?,
                submit: optional_bool(args, "submit")?.unwrap_or(false),
            }),
            _ => Err(format!("Unknown tool `{name}`")),
        }
    }
}

fn required_string(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(format!("`{key}` must be a string")),
        None => Err(format!("`{key}` is required")),
    }
}

fn required_u64(args: &Map<String, Value>, key: &str) -> Result<u64, String> {
    optional_u64(args, key)?.ok_or_else(|| format!("`{key}` is required"))
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
#[path = "command_tests.rs"]
mod tests;
