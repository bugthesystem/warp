//! DevTools commands that deliver [`InputEvent`]s to a page.

use serde_json::{Value, json};

use crate::input::{InputEvent, Key, Modifiers, NamedKey, PointerButton};

/// The DevTools commands for `event` on a page laid out at `viewport` CSS pixels.
pub(crate) fn commands(event: &InputEvent, viewport: (u32, u32)) -> Vec<(&'static str, Value)> {
    let to_page = |at: pathfinder_geometry::vector::Vector2F| {
        (
            f64::from(at.x().clamp(0., 1.)) * f64::from(viewport.0),
            f64::from(at.y().clamp(0., 1.)) * f64::from(viewport.1),
        )
    };
    match event {
        InputEvent::Move { at } => {
            let (x, y) = to_page(*at);
            vec![mouse(
                "mouseMoved",
                x,
                y,
                "none",
                0,
                0,
                Modifiers::default(),
            )]
        }
        InputEvent::Drag { at } => {
            let (x, y) = to_page(*at);
            vec![mouse(
                "mouseMoved",
                x,
                y,
                "left",
                1,
                0,
                Modifiers::default(),
            )]
        }
        InputEvent::Down {
            button,
            at,
            modifiers,
        } => {
            let (x, y) = to_page(*at);
            vec![
                mouse("mouseMoved", x, y, "none", 0, 0, *modifiers),
                mouse(
                    "mousePressed",
                    x,
                    y,
                    button_name(*button),
                    button_mask(*button),
                    1,
                    *modifiers,
                ),
            ]
        }
        InputEvent::Up { button, at } => {
            let (x, y) = to_page(*at);
            vec![mouse(
                "mouseReleased",
                x,
                y,
                button_name(*button),
                0,
                1,
                Modifiers::default(),
            )]
        }
        InputEvent::Scroll { at, delta } => {
            let (x, y) = to_page(*at);
            vec![(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseWheel",
                    "x": x,
                    "y": y,
                    "deltaX": delta.x(),
                    "deltaY": delta.y(),
                }),
            )]
        }
        InputEvent::Key { key, modifiers } => key_commands(*key, *modifiers),
        InputEvent::Text(text) => vec![("Input.insertText", json!({ "text": text }))],
    }
}

fn mouse(
    kind: &str,
    x: f64,
    y: f64,
    button: &str,
    buttons: u8,
    click_count: u8,
    modifiers: Modifiers,
) -> (&'static str, Value) {
    (
        "Input.dispatchMouseEvent",
        json!({
            "type": kind,
            "x": x,
            "y": y,
            "button": button,
            "buttons": buttons,
            "clickCount": click_count,
            "modifiers": modifier_mask(modifiers),
        }),
    )
}

fn button_name(button: PointerButton) -> &'static str {
    match button {
        PointerButton::Left => "left",
        PointerButton::Right => "right",
        PointerButton::Middle => "middle",
    }
}

fn button_mask(button: PointerButton) -> u8 {
    match button {
        PointerButton::Left => 1,
        PointerButton::Right => 2,
        PointerButton::Middle => 4,
    }
}

fn modifier_mask(modifiers: Modifiers) -> u8 {
    u8::from(modifiers.alt)
        | u8::from(modifiers.ctrl) << 1
        | u8::from(modifiers.cmd) << 2
        | u8::from(modifiers.shift) << 3
}

fn key_commands(key: Key, modifiers: Modifiers) -> Vec<(&'static str, Value)> {
    let (name, code, virtual_key, text) = match key {
        Key::Char(char) => {
            let upper = char.to_ascii_uppercase();
            let code = if upper.is_ascii_alphabetic() {
                format!("Key{upper}")
            } else if char.is_ascii_digit() {
                format!("Digit{char}")
            } else {
                String::new()
            };
            let virtual_key = if upper.is_ascii_alphanumeric() {
                upper as u32
            } else {
                0
            };
            (char.to_string(), code, virtual_key, Some(char.to_string()))
        }
        Key::Named(named) => {
            let (name, virtual_key, text) = named_key(named);
            let code = match named {
                NamedKey::Space => "Space".to_owned(),
                _ => name.clone(),
            };
            (name, code, virtual_key, text)
        }
    };
    // Shortcuts like cmd+a must not also insert their character.
    let text = text.filter(|_| !(modifiers.cmd || modifiers.ctrl || modifiers.alt));
    let down_type = if text.is_some() {
        "keyDown"
    } else {
        "rawKeyDown"
    };
    let mut down = json!({
        "type": down_type,
        "key": name,
        "code": code,
        "windowsVirtualKeyCode": virtual_key,
        "modifiers": modifier_mask(modifiers),
    });
    if let Some(text) = text {
        down["text"] = json!(text);
    }
    let up = json!({
        "type": "keyUp",
        "key": name,
        "code": code,
        "windowsVirtualKeyCode": virtual_key,
        "modifiers": modifier_mask(modifiers),
    });
    vec![
        ("Input.dispatchKeyEvent", down),
        ("Input.dispatchKeyEvent", up),
    ]
}

/// The DOM key name, Windows virtual key code and typed text of `key`.
fn named_key(key: NamedKey) -> (String, u32, Option<String>) {
    let (name, virtual_key, text): (&str, u32, Option<&str>) = match key {
        NamedKey::Enter => ("Enter", 13, Some("\r")),
        NamedKey::Tab => ("Tab", 9, None),
        NamedKey::Space => (" ", 32, Some(" ")),
        NamedKey::Backspace => ("Backspace", 8, None),
        NamedKey::Escape => ("Escape", 27, None),
        NamedKey::Delete => ("Delete", 46, None),
        NamedKey::Home => ("Home", 36, None),
        NamedKey::End => ("End", 35, None),
        NamedKey::PageUp => ("PageUp", 33, None),
        NamedKey::PageDown => ("PageDown", 34, None),
        NamedKey::Left => ("ArrowLeft", 37, None),
        NamedKey::Up => ("ArrowUp", 38, None),
        NamedKey::Right => ("ArrowRight", 39, None),
        NamedKey::Down => ("ArrowDown", 40, None),
        NamedKey::F(number) => {
            let number = number.clamp(1, 12);
            return (format!("F{number}"), 111 + u32::from(number), None);
        }
    };
    (name.to_owned(), virtual_key, text.map(str::to_owned))
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
