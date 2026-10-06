//! The preview tools agents get beside the browser tools, on the same local MCP endpoint.
//!
//! The tools list what can be shown, open a preview, look at it, and click, drag, scroll and type
//! in it. Editing a scene stays with the engine's own MCP server.

use pathfinder_geometry::vector::{Vector2F, vec2f};
use serde_json::{Map, Value, json};

use crate::input::{InputEvent, Key, Modifiers, PointerButton};

/// Longest side of the picture a tool returns unless the agent asks for full size.
pub const SMALL_PICTURE_SIDE: u32 = 1024;

/// How many log lines `preview_look` returns by default.
pub const DEFAULT_LOG_LINES: usize = 40;

/// How many pointer moves an agent's drag is split into, so targets see a drag rather than a jump.
const DRAG_STEPS: u32 = 8;

pub const SERVER_INSTRUCTIONS: &str = "Preview tools show other programs live in a Warp preview \
pane: a web page in a headless browser, or another app's window such as a game engine, a game or \
a simulator. Call preview_targets to see what can be shown, preview_open to show it, then \
preview_look to see the picture and recent log in one call. preview_click, preview_drag, \
preview_scroll, preview_type and preview_key act on the preview the way the user's pointer and \
keyboard would, without moving the user's own pointer; their x and y are pixels in the picture \
preview_look returns, and each replies with a picture of the result. Change scenes and code \
through the engine's own MCP server or the shell, then look again.";

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
    Click {
        preview: Option<u64>,
        at: (u32, u32),
        button: PointerButton,
        double: bool,
    },
    Drag {
        preview: Option<u64>,
        from: (u32, u32),
        to: (u32, u32),
    },
    Scroll {
        preview: Option<u64>,
        at: (u32, u32),
        delta: (i32, i32),
    },
    Type {
        preview: Option<u64>,
        text: String,
    },
    Key {
        preview: Option<u64>,
        key: Key,
        modifiers: Modifiers,
        /// The chord as the agent wrote it, for the step shown to the user.
        chord: String,
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
            Self::Click { .. } => Some("Clicking in the preview".to_owned()),
            Self::Drag { .. } => Some("Dragging in the preview".to_owned()),
            Self::Scroll { .. } => Some("Scrolling the preview".to_owned()),
            Self::Type { .. } => Some("Typing in the preview".to_owned()),
            Self::Key { chord, .. } => Some(format!("Pressing {chord} in the preview")),
        }
    }

    /// The preview a command acts on, when it names one.
    pub fn preview(&self) -> Option<u64> {
        match self {
            Self::Targets | Self::Open(_) => None,
            Self::Screenshot { preview, .. }
            | Self::Look { preview, .. }
            | Self::Click { preview, .. }
            | Self::Drag { preview, .. }
            | Self::Scroll { preview, .. }
            | Self::Type { preview, .. }
            | Self::Key { preview, .. } => *preview,
        }
    }

    /// Whether the command acts on a preview rather than only looking.
    pub fn acts(&self) -> bool {
        match self {
            Self::Targets | Self::Open(_) | Self::Screenshot { .. } | Self::Look { .. } => false,
            Self::Click { .. }
            | Self::Drag { .. }
            | Self::Scroll { .. }
            | Self::Type { .. }
            | Self::Key { .. } => true,
        }
    }

    /// The input an acting command delivers, for a preview whose agent-facing picture is
    /// `picture` pixels. `None` for commands that only look. The error is a message for the agent.
    pub fn input_events(&self, picture: (u32, u32)) -> Option<Result<Vec<InputEvent>, String>> {
        let fraction = |(x, y): (u32, u32)| -> Result<Vector2F, String> {
            let (width, height) = picture;
            if x >= width || y >= height {
                return Err(format!(
                    "({x}, {y}) is outside the picture, which is {width}x{height} pixels"
                ));
            }
            Ok(vec2f(
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            ))
        };
        let events = match self {
            Self::Targets | Self::Open(_) | Self::Screenshot { .. } | Self::Look { .. } => {
                return None;
            }
            Self::Click {
                at, button, double, ..
            } => fraction(*at).map(|at| {
                let presses = if *double { 2 } else { 1 };
                (0..presses)
                    .flat_map(|_| {
                        [
                            InputEvent::Down {
                                button: *button,
                                at,
                                modifiers: Modifiers::default(),
                            },
                            InputEvent::Up {
                                button: *button,
                                at,
                            },
                        ]
                    })
                    .collect()
            }),
            Self::Drag { from, to, .. } => fraction(*from).and_then(|from| {
                let to = fraction(*to)?;
                let mut events = vec![InputEvent::Down {
                    button: PointerButton::Left,
                    at: from,
                    modifiers: Modifiers::default(),
                }];
                events.extend((1..=DRAG_STEPS).map(|step| InputEvent::Drag {
                    at: from + (to - from) * (step as f32 / DRAG_STEPS as f32),
                }));
                events.push(InputEvent::Up {
                    button: PointerButton::Left,
                    at: to,
                });
                Ok(events)
            }),
            Self::Scroll { at, delta, .. } => fraction(*at).map(|at| {
                vec![InputEvent::Scroll {
                    at,
                    delta: vec2f(delta.0 as f32, delta.1 as f32),
                }]
            }),
            Self::Type { text, .. } => Ok(vec![InputEvent::Text(text.clone())]),
            Self::Key { key, modifiers, .. } => Ok(vec![InputEvent::Key {
                key: *key,
                modifiers: *modifiers,
            }]),
        };
        Some(events)
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
            "preview_click" => Ok(Self::Click {
                preview,
                at: point(args, "x", "y")?,
                button: match optional_string(args, "button")?.as_deref() {
                    None | Some("left") => PointerButton::Left,
                    Some("right") => PointerButton::Right,
                    Some("middle") => PointerButton::Middle,
                    Some(other) => {
                        return Err(format!(
                            "`button` must be left, right or middle, not `{other}`"
                        ));
                    }
                },
                double: optional_bool(args, "double")?.unwrap_or(false),
            }),
            "preview_drag" => Ok(Self::Drag {
                preview,
                from: point(args, "from_x", "from_y")?,
                to: point(args, "to_x", "to_y")?,
            }),
            "preview_scroll" => {
                let delta = (
                    optional_i32(args, "dx")?.unwrap_or(0),
                    optional_i32(args, "dy")?.unwrap_or(0),
                );
                if delta == (0, 0) {
                    return Err("Pass `dx` or `dy`".to_owned());
                }
                Ok(Self::Scroll {
                    preview,
                    at: point(args, "x", "y")?,
                    delta,
                })
            }
            "preview_type" => match optional_string(args, "text")? {
                Some(text) if !text.is_empty() => Ok(Self::Type { preview, text }),
                _ => Err("Pass the `text` to type".to_owned()),
            },
            "preview_key" => {
                let chord = optional_string(args, "key")?.ok_or("Pass the `key` to press")?;
                let (key, modifiers) = Key::parse_chord(&chord).ok_or_else(|| {
                    format!(
                        "`{chord}` is not a key: use a character or a name such as enter, tab, \
                         escape, up or f5, with cmd+, shift+, alt+ or ctrl+ in front"
                    )
                })?;
                Ok(Self::Key {
                    preview,
                    key,
                    modifiers,
                    chord,
                })
            }
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
        (
            "preview_click",
            "Click in the preview at x, y: pixels in the picture preview_look returns. Replies \
             with a picture of the result.",
            json!({
                "preview": preview,
                "x": {"type": "integer", "minimum": 0},
                "y": {"type": "integer", "minimum": 0},
                "button": {"type": "string", "enum": ["left", "right", "middle"], "description": "Defaults to left."},
                "double": {"type": "boolean", "description": "Double-click. Defaults to false."}
            }),
            &["x", "y"],
        ),
        (
            "preview_drag",
            "Press the left button at from_x, from_y, move to to_x, to_y and release: pixels in \
             the picture preview_look returns. Replies with a picture of the result.",
            json!({
                "preview": preview,
                "from_x": {"type": "integer", "minimum": 0},
                "from_y": {"type": "integer", "minimum": 0},
                "to_x": {"type": "integer", "minimum": 0},
                "to_y": {"type": "integer", "minimum": 0}
            }),
            &["from_x", "from_y", "to_x", "to_y"],
        ),
        (
            "preview_scroll",
            "Scroll the preview at x, y by dx, dy pixels; positive dy scrolls down. Replies with \
             a picture of the result.",
            json!({
                "preview": preview,
                "x": {"type": "integer", "minimum": 0},
                "y": {"type": "integer", "minimum": 0},
                "dx": {"type": "integer"},
                "dy": {"type": "integer"}
            }),
            &["x", "y"],
        ),
        (
            "preview_type",
            "Type text into whatever has focus in the preview. Click a field first. Replies with \
             a picture of the result.",
            json!({
                "preview": preview,
                "text": {"type": "string"}
            }),
            &["text"],
        ),
        (
            "preview_key",
            "Press a key or chord in the preview, such as enter, escape, up, f5, space or \
             cmd+s. Replies with a picture of the result.",
            json!({
                "preview": preview,
                "key": {"type": "string"}
            }),
            &["key"],
        ),
    ]
}

/// A required point given as two non-negative integer arguments.
fn point(args: &Map<String, Value>, x: &str, y: &str) -> Result<(u32, u32), String> {
    let coordinate = |key: &str| -> Result<u32, String> {
        let value = optional_u64(args, key)?.ok_or_else(|| format!("Pass `{key}`"))?;
        u32::try_from(value).map_err(|_| format!("`{key}` is too large"))
    };
    Ok((coordinate(x)?, coordinate(y)?))
}

fn optional_i32(args: &Map<String, Value>, key: &str) -> Result<Option<i32>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| format!("`{key}` must be an integer")),
    }
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
