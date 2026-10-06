use pathfinder_geometry::vector::vec2f;
use serde_json::json;

use super::commands;
use crate::input::{InputEvent, Key, Modifiers, NamedKey, PointerButton};

const VIEWPORT: (u32, u32) = (1000, 500);

#[test]
fn maps_picture_fractions_to_page_pixels() {
    let commands = commands(
        &InputEvent::Down {
            button: PointerButton::Left,
            at: vec2f(0.5, 0.25),
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
        },
        VIEWPORT,
    );
    assert_eq!(commands.len(), 2);
    let (method, pressed) = &commands[1];
    assert_eq!(*method, "Input.dispatchMouseEvent");
    assert_eq!(
        *pressed,
        json!({
            "type": "mousePressed",
            "x": 500.0,
            "y": 125.0,
            "button": "left",
            "buttons": 1,
            "clickCount": 1,
            "modifiers": 8,
        })
    );
}

#[test]
fn clamps_points_outside_the_picture() {
    let commands = commands(
        &InputEvent::Up {
            button: PointerButton::Right,
            at: vec2f(1.5, -0.2),
        },
        VIEWPORT,
    );
    assert_eq!(commands[0].1["x"], json!(1000.0));
    assert_eq!(commands[0].1["y"], json!(0.0));
    assert_eq!(commands[0].1["button"], json!("right"));
}

#[test]
fn scrolls_by_the_given_delta() {
    let commands = commands(
        &InputEvent::Scroll {
            at: vec2f(0., 1.),
            delta: vec2f(0., 120.),
        },
        VIEWPORT,
    );
    assert_eq!(commands[0].1["type"], json!("mouseWheel"));
    assert_eq!(commands[0].1["deltaY"], json!(120.0));
    assert_eq!(commands[0].1["y"], json!(500.0));
}

#[test]
fn types_characters_and_presses_shortcuts_without_text() {
    let typed = commands(
        &InputEvent::Key {
            key: Key::Char('a'),
            modifiers: Modifiers::default(),
        },
        VIEWPORT,
    );
    assert_eq!(typed[0].1["type"], json!("keyDown"));
    assert_eq!(typed[0].1["text"], json!("a"));
    assert_eq!(typed[0].1["code"], json!("KeyA"));
    assert_eq!(typed[0].1["windowsVirtualKeyCode"], json!(65));
    assert_eq!(typed[1].1["type"], json!("keyUp"));

    let shortcut = commands(
        &InputEvent::Key {
            key: Key::Char('a'),
            modifiers: Modifiers {
                cmd: true,
                ..Default::default()
            },
        },
        VIEWPORT,
    );
    assert_eq!(shortcut[0].1["type"], json!("rawKeyDown"));
    assert!(shortcut[0].1.get("text").is_none());
    assert_eq!(shortcut[0].1["modifiers"], json!(4));
}

#[test]
fn presses_named_keys() {
    let enter = commands(
        &InputEvent::Key {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        },
        VIEWPORT,
    );
    assert_eq!(enter[0].1["key"], json!("Enter"));
    assert_eq!(enter[0].1["text"], json!("\r"));
    assert_eq!(enter[0].1["windowsVirtualKeyCode"], json!(13));

    let f5 = commands(
        &InputEvent::Key {
            key: Key::Named(NamedKey::F(5)),
            modifiers: Modifiers::default(),
        },
        VIEWPORT,
    );
    assert_eq!(f5[0].1["type"], json!("rawKeyDown"));
    assert_eq!(f5[0].1["key"], json!("F5"));
    assert_eq!(f5[0].1["windowsVirtualKeyCode"], json!(116));
}

#[test]
fn inserts_text() {
    let commands = commands(&InputEvent::Text("héllo".to_owned()), VIEWPORT);
    assert_eq!(
        commands,
        vec![("Input.insertText", json!({ "text": "héllo" }))]
    );
}
