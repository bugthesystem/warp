use serde_json::{Map, Value, json};

use super::*;

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("an object")
}

#[test]
fn parses_open_by_url_or_window() {
    assert_eq!(
        PreviewCommand::from_tool_call("preview_open", &args(json!({"url": "http://localhost:5173"}))),
        Ok(PreviewCommand::Open(OpenTarget::Url("http://localhost:5173".to_owned())))
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
        assert!(required.is_empty());
        let call = if name == "preview_open" {
            args(json!({"url": "http://localhost"}))
        } else {
            Map::new()
        };
        assert!(PreviewCommand::from_tool_call(name, &call).is_ok(), "{name}");
    }
}
