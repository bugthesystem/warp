//! What crosses the line to Baguette and simctl: the device list, the MJPEG frame stream, and the
//! JSON gestures `baguette input` reads.

use std::io::{self, BufRead};

use pathfinder_geometry::vector::Vector2F;
use serde_json::{Value, json};

use super::SimulatorEntry;
use crate::SimulatorSource;
use crate::input::{InputEvent, Key, Modifiers, NamedKey, PointerButton};

/// How long a scroll's swipe takes, in seconds.
const SCROLL_SWIPE_DURATION: f64 = 0.15;

/// Parses `xcrun simctl list devices available -j` into iOS devices, booted ones first, then by
/// name.
pub fn parse_simctl_devices(json: &str) -> Vec<SimulatorEntry> {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(Value::Object(runtimes)) = root.get("devices") else {
        return Vec::new();
    };
    let mut entries: Vec<SimulatorEntry> = runtimes
        .iter()
        .filter_map(|(runtime, devices)| Some((runtime_name(runtime)?, devices.as_array()?)))
        .flat_map(|(runtime, devices)| {
            devices.iter().filter_map(move |device| {
                if device["isAvailable"].as_bool() == Some(false) {
                    return None;
                }
                Some(SimulatorEntry {
                    source: SimulatorSource {
                        udid: device["udid"].as_str()?.to_owned(),
                        name: device["name"].as_str()?.to_owned(),
                    },
                    runtime: runtime.clone(),
                    booted: device["state"].as_str() == Some("Booted"),
                })
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.booted
            .cmp(&a.booted)
            .then_with(|| a.source.name.cmp(&b.source.name))
            .then_with(|| b.runtime.cmp(&a.runtime))
    });
    entries
}

/// "iOS 26.5" for `com.apple.CoreSimulator.SimRuntime.iOS-26-5`. `None` for other platforms,
/// such as watchOS and tvOS, whose devices take input differently.
fn runtime_name(identifier: &str) -> Option<String> {
    let version = identifier.rsplit('.').next()?.strip_prefix("iOS-")?;
    Some(format!("iOS {}", version.replace('-', ".")))
}

/// Reads the JPEG frames of a `multipart/x-mixed-replace` stream, where each part has a
/// `Content-Length` header.
pub(crate) struct MultipartReader<R> {
    reader: R,
}

impl<R: BufRead> MultipartReader<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self { reader }
    }

    /// The next part's bytes, or `None` at the end of the stream.
    pub(crate) fn next_part(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut length = None;
        let mut line = String::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            let header = line.trim_end();
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse::<usize>().ok();
            } else if header.is_empty()
                && let Some(length) = length.take()
            {
                let mut part = vec![0; length];
                self.reader.read_exact(&mut part)?;
                return Ok(Some(part));
            }
        }
    }
}

/// The `baguette input` lines for `event` on a device whose screen is `screen` points.
pub(crate) fn input_lines(event: &InputEvent, screen: (f64, f64)) -> Vec<String> {
    let (width, height) = screen;
    let to_screen = |at: Vector2F| {
        (
            f64::from(at.x().clamp(0., 1.)) * width,
            f64::from(at.y().clamp(0., 1.)) * height,
        )
    };
    let gesture = |mut envelope: Value| {
        envelope["width"] = json!(width);
        envelope["height"] = json!(height);
        envelope.to_string()
    };
    match event {
        InputEvent::Down {
            button: PointerButton::Left,
            at,
            ..
        } => {
            let (x, y) = to_screen(*at);
            vec![gesture(json!({"type": "touch1-down", "x": x, "y": y}))]
        }
        InputEvent::Drag { at } => {
            let (x, y) = to_screen(*at);
            vec![gesture(json!({"type": "touch1-move", "x": x, "y": y}))]
        }
        InputEvent::Up {
            button: PointerButton::Left,
            at,
        } => {
            let (x, y) = to_screen(*at);
            vec![gesture(json!({"type": "touch1-up", "x": x, "y": y}))]
        }
        // A touchscreen has one kind of touch, so other buttons tap where they are pressed.
        InputEvent::Down { at, .. } => {
            let (x, y) = to_screen(*at);
            vec![gesture(
                json!({"type": "tap", "x": x, "y": y, "duration": 0.05}),
            )]
        }
        // A touchscreen has no pointer to move.
        InputEvent::Up { .. } | InputEvent::Move { .. } => Vec::new(),
        InputEvent::Scroll { at, delta } => {
            let (x, y) = to_screen(*at);
            // Content scrolls down when the finger moves up.
            let end_x = (x - f64::from(delta.x())).clamp(0., width);
            let end_y = (y - f64::from(delta.y())).clamp(0., height);
            vec![gesture(json!({
                "type": "swipe",
                "startX": x,
                "startY": y,
                "endX": end_x,
                "endY": end_y,
                "duration": SCROLL_SWIPE_DURATION,
            }))]
        }
        InputEvent::Key { key, modifiers } => match key_code(*key) {
            Some(code) => {
                let mut envelope = json!({"type": "key", "code": code});
                let modifiers = modifier_names(*modifiers);
                if !modifiers.is_empty() {
                    envelope["modifiers"] = json!(modifiers);
                }
                vec![envelope.to_string()]
            }
            None => match key {
                Key::Char(char) => {
                    vec![json!({"type": "type", "text": char.to_string()}).to_string()]
                }
                Key::Named(_) => Vec::new(),
            },
        },
        InputEvent::Text(text) => vec![json!({"type": "type", "text": text}).to_string()],
    }
}

/// The W3C `KeyboardEvent.code` Baguette presses for `key`, when it has one.
fn key_code(key: Key) -> Option<String> {
    match key {
        Key::Char(char) if char.is_ascii_alphabetic() => {
            Some(format!("Key{}", char.to_ascii_uppercase()))
        }
        Key::Char(char) if char.is_ascii_digit() => Some(format!("Digit{char}")),
        Key::Char(_) => None,
        Key::Named(named) => match named {
            NamedKey::Enter => Some("Enter".to_owned()),
            NamedKey::Tab => Some("Tab".to_owned()),
            NamedKey::Space => Some("Space".to_owned()),
            NamedKey::Backspace | NamedKey::Delete => Some("Backspace".to_owned()),
            NamedKey::Escape => Some("Escape".to_owned()),
            NamedKey::Left => Some("ArrowLeft".to_owned()),
            NamedKey::Right => Some("ArrowRight".to_owned()),
            NamedKey::Up => Some("ArrowUp".to_owned()),
            NamedKey::Down => Some("ArrowDown".to_owned()),
            NamedKey::Home | NamedKey::End | NamedKey::PageUp | NamedKey::PageDown => None,
            NamedKey::F(_) => None,
        },
    }
}

fn modifier_names(modifiers: Modifiers) -> Vec<&'static str> {
    [
        (modifiers.shift, "shift"),
        (modifiers.ctrl, "control"),
        (modifiers.alt, "option"),
        (modifiers.cmd, "command"),
    ]
    .into_iter()
    .filter_map(|(held, name)| held.then_some(name))
    .collect()
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
