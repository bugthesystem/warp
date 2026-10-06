use std::io::Cursor;

use pathfinder_geometry::vector::vec2f;
use serde_json::{Value, json};

use super::{MultipartReader, input_lines, parse_simctl_devices};
use crate::input::{InputEvent, Key, Modifiers, NamedKey, PointerButton};

const SCREEN: (f64, f64) = (400., 800.);

fn parsed(lines: Vec<String>) -> Vec<Value> {
    lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect()
}

#[test]
fn lists_available_ios_devices_booted_first() {
    let json = json!({
        "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                {"udid": "B", "name": "iPhone 17", "state": "Shutdown", "isAvailable": true},
                {"udid": "A", "name": "iPhone 17 Pro", "state": "Booted", "isAvailable": true},
                {"udid": "C", "name": "iPhone 16", "state": "Shutdown", "isAvailable": false}
            ],
            "com.apple.CoreSimulator.SimRuntime.watchOS-11-0": [
                {"udid": "W", "name": "Apple Watch", "state": "Booted", "isAvailable": true}
            ]
        }
    })
    .to_string();
    let entries = parse_simctl_devices(&json);
    let names: Vec<&str> = entries
        .iter()
        .map(|entry| entry.source.name.as_str())
        .collect();
    assert_eq!(names, vec!["iPhone 17 Pro", "iPhone 17"]);
    assert!(entries[0].booted);
    assert_eq!(entries[0].runtime, "iOS 26.5");
    assert!(parse_simctl_devices("not json").is_empty());
}

#[test]
fn reads_jpeg_parts_from_a_multipart_stream() {
    let mut stream = Vec::new();
    stream.extend_from_slice(
        b"HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=frame\r\n\r\n",
    );
    for part in [&b"first"[..], &b"second\r\n--frame"[..]] {
        stream.extend_from_slice(b"--frame\r\nContent-Type: image/jpeg\r\n");
        stream.extend_from_slice(format!("Content-Length: {}\r\n\r\n", part.len()).as_bytes());
        stream.extend_from_slice(part);
        stream.extend_from_slice(b"\r\n");
    }
    let mut reader = MultipartReader::new(Cursor::new(stream));
    assert_eq!(reader.next_part().unwrap(), Some(b"first".to_vec()));
    assert_eq!(
        reader.next_part().unwrap(),
        Some(b"second\r\n--frame".to_vec()),
        "a part's bytes are read by length, even when they look like a boundary"
    );
    assert_eq!(reader.next_part().unwrap(), None);
}

#[test]
fn touches_follow_a_left_button_drag_in_screen_points() {
    let lines = parsed(
        [
            InputEvent::Down {
                button: PointerButton::Left,
                at: vec2f(0.5, 0.25),
                modifiers: Modifiers::default(),
            },
            InputEvent::Drag {
                at: vec2f(0.5, 0.5),
            },
            InputEvent::Up {
                button: PointerButton::Left,
                at: vec2f(0.5, 1.5),
            },
        ]
        .iter()
        .flat_map(|event| input_lines(event, SCREEN))
        .collect(),
    );
    assert_eq!(
        lines[0],
        json!({"type": "touch1-down", "x": 200.0, "y": 200.0, "width": 400.0, "height": 800.0})
    );
    assert_eq!(lines[1]["type"], json!("touch1-move"));
    assert_eq!(lines[1]["y"], json!(400.0));
    assert_eq!(
        lines[2]["y"],
        json!(800.0),
        "points off the picture are clamped"
    );
}

#[test]
fn other_buttons_tap() {
    let down = parsed(input_lines(
        &InputEvent::Down {
            button: PointerButton::Right,
            at: vec2f(0., 0.),
            modifiers: Modifiers::default(),
        },
        SCREEN,
    ));
    assert_eq!(down[0]["type"], json!("tap"));
    assert!(
        input_lines(
            &InputEvent::Up {
                button: PointerButton::Right,
                at: vec2f(0., 0.),
            },
            SCREEN,
        )
        .is_empty()
    );
}

#[test]
fn scrolls_by_swiping_the_other_way() {
    let swipe = parsed(input_lines(
        &InputEvent::Scroll {
            at: vec2f(0.5, 0.5),
            delta: vec2f(0., 100.),
        },
        SCREEN,
    ));
    assert_eq!(swipe[0]["type"], json!("swipe"));
    assert_eq!(swipe[0]["startY"], json!(400.0));
    assert_eq!(swipe[0]["endY"], json!(300.0));
}

#[test]
fn presses_keys_by_code_and_types_other_characters() {
    let key = parsed(input_lines(
        &InputEvent::Key {
            key: Key::Char('a'),
            modifiers: Modifiers {
                cmd: true,
                shift: true,
                ..Default::default()
            },
        },
        SCREEN,
    ));
    assert_eq!(
        key[0],
        json!({"type": "key", "code": "KeyA", "modifiers": ["shift", "command"]})
    );
    let enter = parsed(input_lines(
        &InputEvent::Key {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        },
        SCREEN,
    ));
    assert_eq!(enter[0], json!({"type": "key", "code": "Enter"}));
    let symbol = parsed(input_lines(
        &InputEvent::Key {
            key: Key::Char('@'),
            modifiers: Modifiers::default(),
        },
        SCREEN,
    ));
    assert_eq!(symbol[0], json!({"type": "type", "text": "@"}));
    assert!(
        input_lines(
            &InputEvent::Key {
                key: Key::Named(NamedKey::F(5)),
                modifiers: Modifiers::default(),
            },
            SCREEN,
        )
        .is_empty()
    );
    let text = parsed(input_lines(&InputEvent::Text("hi".to_owned()), SCREEN));
    assert_eq!(text[0], json!({"type": "type", "text": "hi"}));
}
