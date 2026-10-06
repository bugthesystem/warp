use serde_json::json;

use super::*;

#[test]
fn reads_responses_and_errors() {
    assert_eq!(
        parse_message(r#"{"id": 3, "result": {"targetId": "T"}}"#),
        Some(Message::Response {
            id: 3,
            result: Ok(json!({"targetId": "T"})),
        })
    );
    assert_eq!(
        parse_message(r#"{"id": 4, "error": {"code": -32000, "message": "No target"}}"#),
        Some(Message::Response {
            id: 4,
            result: Err("No target".to_owned()),
        })
    );
}

#[test]
fn reads_events_with_their_session() {
    assert_eq!(
        parse_message(
            r#"{"method": "Page.screencastFrame", "params": {"sessionId": 1}, "sessionId": "S"}"#
        ),
        Some(Message::Event {
            method: "Page.screencastFrame".to_owned(),
            params: json!({"sessionId": 1}),
            session: Some("S".to_owned()),
        })
    );
    assert_eq!(parse_message("[]"), None);
    assert_eq!(parse_message("not json"), None);
}

#[test]
fn writes_commands_with_an_optional_session() {
    let without: serde_json::Value = serde_json::from_str(&command_message(
        1,
        "Target.createTarget",
        json!({"url": "about:blank"}),
        None,
    ))
    .expect("json");
    assert_eq!(
        without,
        json!({"id": 1, "method": "Target.createTarget", "params": {"url": "about:blank"}})
    );

    let with: serde_json::Value =
        serde_json::from_str(&command_message(2, "Page.enable", json!({}), Some("S")))
            .expect("json");
    assert_eq!(with["sessionId"], json!("S"));
}

#[test]
fn finds_the_devtools_url_in_startup_output() {
    assert_eq!(
        devtools_url("DevTools listening on ws://127.0.0.1:41235/devtools/browser/abc\n"),
        Some("ws://127.0.0.1:41235/devtools/browser/abc")
    );
    assert_eq!(devtools_url("[1006/060000.000:INFO] something else"), None);
}
