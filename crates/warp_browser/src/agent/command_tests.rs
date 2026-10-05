use serde_json::{Map, Value, json};

use super::BrowserCommand;

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap()
}

#[test]
fn parses_open_with_url() {
    let command = BrowserCommand::from_tool_call("browser_open", &args(json!({"url": "warp.dev"})));

    assert_eq!(
        command,
        Ok(BrowserCommand::Open {
            url: "warp.dev".to_owned()
        })
    );
}

#[test]
fn tab_is_optional() {
    let command = BrowserCommand::from_tool_call("browser_read", &args(json!({})));

    assert_eq!(command, Ok(BrowserCommand::Read { tab: None }));
}

#[test]
fn parses_type_with_all_arguments() {
    let command = BrowserCommand::from_tool_call(
        "browser_type",
        &args(json!({"tab": 2, "element": 7, "text": "hello", "submit": true})),
    );

    assert_eq!(
        command,
        Ok(BrowserCommand::Type {
            tab: Some(2),
            element: 7,
            text: "hello".to_owned(),
            submit: true,
        })
    );
}

#[test]
fn type_does_not_submit_by_default() {
    let command =
        BrowserCommand::from_tool_call("browser_type", &args(json!({"element": 1, "text": "hi"})));

    assert_eq!(
        command,
        Ok(BrowserCommand::Type {
            tab: None,
            element: 1,
            text: "hi".to_owned(),
            submit: false,
        })
    );
}

#[test]
fn missing_required_argument_is_an_error() {
    let command = BrowserCommand::from_tool_call("browser_click", &args(json!({})));

    assert_eq!(command, Err("`element` is required".to_owned()));
}

#[test]
fn wrongly_typed_argument_is_an_error() {
    let command = BrowserCommand::from_tool_call("browser_click", &args(json!({"element": "3"})));

    assert_eq!(
        command,
        Err("`element` must be a non-negative integer".to_owned())
    );
}

#[test]
fn unknown_tool_is_an_error() {
    let command = BrowserCommand::from_tool_call("browser_fly", &args(json!({})));

    assert_eq!(command, Err("Unknown tool `browser_fly`".to_owned()));
}

#[test]
fn console_does_not_clear_by_default() {
    let command = BrowserCommand::from_tool_call("browser_console", &args(json!({"tab": 1})));

    assert_eq!(
        command,
        Ok(BrowserCommand::Console {
            tab: Some(1),
            clear: false,
        })
    );
}

#[test]
fn tab_is_reported_for_commands_that_act_on_a_tab() {
    let click = BrowserCommand::Click {
        tab: Some(3),
        element: 1,
    };
    let open = BrowserCommand::Open {
        url: "warp.dev".to_owned(),
    };

    assert_eq!(click.tab(), Some(3));
    assert_eq!(open.tab(), None);
}
