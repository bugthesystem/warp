use serde_json::{Map, Value, json};

use super::*;
use crate::input::{InputEvent, Key, Modifiers, PointerButton};

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("an object")
}

#[test]
fn parses_open_by_url_or_window() {
    assert_eq!(
        PreviewCommand::from_tool_call(
            "preview_open",
            &args(json!({"url": "http://localhost:5173"}))
        ),
        Ok(PreviewCommand::Open(OpenTarget::Url(
            "http://localhost:5173".to_owned()
        )))
    );
    assert_eq!(
        PreviewCommand::from_tool_call("preview_open", &args(json!({"window": 812}))),
        Ok(PreviewCommand::Open(OpenTarget::Window(812)))
    );
}

#[test]
fn open_needs_exactly_one_target() {
    assert!(PreviewCommand::from_tool_call("preview_open", &args(json!({}))).is_err());
    assert!(
        PreviewCommand::from_tool_call(
            "preview_open",
            &args(json!({"url": "http://localhost", "window": 1}))
        )
        .is_err()
    );
    assert!(
        PreviewCommand::from_tool_call("preview_open", &args(json!({"window": u64::MAX}))).is_err()
    );
}

#[test]
fn parses_look_and_screenshot_with_defaults() {
    assert_eq!(
        PreviewCommand::from_tool_call("preview_look", &args(json!({}))),
        Ok(PreviewCommand::Look {
            preview: None,
            lines: DEFAULT_LOG_LINES,
        })
    );
    assert_eq!(
        PreviewCommand::from_tool_call(
            "preview_screenshot",
            &args(json!({"preview": 2, "full_size": true}))
        ),
        Ok(PreviewCommand::Screenshot {
            preview: Some(2),
            full_size: true,
        })
    );
}

#[test]
fn rejects_bad_arguments_and_unknown_tools() {
    assert_eq!(
        PreviewCommand::from_tool_call("preview_look", &args(json!({"preview": "two"}))),
        Err("`preview` must be a non-negative integer".to_owned())
    );
    assert!(PreviewCommand::from_tool_call("preview_click", &args(json!({}))).is_err());
}

#[test]
fn every_defined_tool_parses() {
    for (name, _, _, required) in tool_definitions() {
        assert!(PreviewCommand::is_preview_tool(name));
        let mut call = Map::new();
        for key in required {
            let value = match *key {
                "text" | "key" => json!("a"),
                _ => json!(1),
            };
            call.insert((*key).to_owned(), value);
        }
        match name {
            "preview_open" => {
                call.insert("url".to_owned(), json!("http://localhost"));
            }
            "preview_scroll" => {
                call.insert("dy".to_owned(), json!(1));
            }
            _ => {}
        }
        assert!(
            PreviewCommand::from_tool_call(name, &call).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn parses_click_drag_and_scroll() {
    assert_eq!(
        PreviewCommand::from_tool_call(
            "preview_click",
            &args(json!({"x": 10, "y": 20, "button": "right"}))
        ),
        Ok(PreviewCommand::Click {
            preview: None,
            at: (10, 20),
            button: PointerButton::Right,
            double: false,
        })
    );
    assert_eq!(
        PreviewCommand::from_tool_call(
            "preview_drag",
            &args(json!({"preview": 3, "from_x": 1, "from_y": 2, "to_x": 3, "to_y": 4}))
        ),
        Ok(PreviewCommand::Drag {
            preview: Some(3),
            from: (1, 2),
            to: (3, 4),
        })
    );
    assert_eq!(
        PreviewCommand::from_tool_call(
            "preview_scroll",
            &args(json!({"x": 5, "y": 6, "dy": -120}))
        ),
        Ok(PreviewCommand::Scroll {
            preview: None,
            at: (5, 6),
            delta: (0, -120),
        })
    );
}

#[test]
fn rejects_bad_input_arguments() {
    for (name, value) in [
        ("preview_click", json!({"x": 1})),
        ("preview_click", json!({"x": 1, "y": 1, "button": "back"})),
        ("preview_scroll", json!({"x": 1, "y": 1})),
        ("preview_type", json!({"text": ""})),
        ("preview_key", json!({})),
        ("preview_key", json!({"key": "hyper+k"})),
    ] {
        assert!(
            PreviewCommand::from_tool_call(name, &args(value.clone())).is_err(),
            "{name} {value}"
        );
    }
}

#[test]
fn parses_key_chords() {
    assert_eq!(
        PreviewCommand::from_tool_call("preview_key", &args(json!({"key": "cmd+s"}))),
        Ok(PreviewCommand::Key {
            preview: None,
            key: Key::Char('s'),
            modifiers: Modifiers {
                cmd: true,
                ..Default::default()
            },
            chord: "cmd+s".to_owned(),
        })
    );
}

#[test]
fn turns_picture_pixels_into_picture_fractions() {
    let click = PreviewCommand::Click {
        preview: None,
        at: (49, 24),
        button: PointerButton::Left,
        double: true,
    };
    let events = click.input_events((100, 50)).unwrap().unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0],
        InputEvent::Down {
            button: PointerButton::Left,
            at: pathfinder_geometry::vector::vec2f(0.495, 0.49),
            modifiers: Modifiers::default(),
        }
    );

    let outside = PreviewCommand::Click {
        preview: None,
        at: (100, 0),
        button: PointerButton::Left,
        double: false,
    };
    assert!(outside.input_events((100, 50)).unwrap().is_err());
    assert!(PreviewCommand::Targets.input_events((100, 50)).is_none());
}

#[test]
fn splits_drags_into_moves_between_press_and_release() {
    let drag = PreviewCommand::Drag {
        preview: None,
        from: (0, 0),
        to: (99, 0),
    };
    let events = drag.input_events((100, 100)).unwrap().unwrap();
    assert!(matches!(events.first(), Some(InputEvent::Down { .. })));
    assert!(matches!(events.last(), Some(InputEvent::Up { .. })));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, InputEvent::Drag { .. }))
            .count(),
        8
    );
}

#[test]
fn only_input_tools_act() {
    assert!(!PreviewCommand::Targets.acts());
    assert!(
        !PreviewCommand::Look {
            preview: None,
            lines: 1
        }
        .acts()
    );
    assert!(
        PreviewCommand::Type {
            preview: None,
            text: "hi".to_owned()
        }
        .acts()
    );
}
