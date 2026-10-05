use serde_json::json;

use super::{click_script, format_console_messages, format_page_snapshot, type_script};

#[test]
fn formats_elements_with_numbers_and_links() {
    let json = json!({
        "url": "http://localhost:5173/",
        "title": "My app",
        "ready": "complete",
        "text": "Welcome",
        "elements": [
            {"id": 1, "kind": "a", "name": "Docs", "href": "http://localhost:5173/docs"},
            {"id": 2, "kind": "button[submit]", "name": "Sign in", "href": null}
        ]
    })
    .to_string();

    let output = format_page_snapshot(&json).unwrap();

    assert_eq!(
        output,
        "Title: My app\n\
         URL: http://localhost:5173/\n\
         \n\
         Interactive elements (use the number with browser_click or browser_type):\n\
         [1] a \"Docs\" -> http://localhost:5173/docs\n\
         [2] button[submit] \"Sign in\"\n\
         \n\
         Page text:\n\
         Welcome"
    );
}

#[test]
fn notes_when_page_is_still_loading() {
    let json = json!({
        "url": "https://warp.dev/",
        "title": "",
        "ready": "interactive",
        "text": "",
        "elements": []
    })
    .to_string();

    let output = format_page_snapshot(&json).unwrap();

    assert!(
        output.contains("Note: the page is still loading (interactive).\n"),
        "{output}"
    );
    assert!(output.contains("(none)\n"), "{output}");
}

#[test]
fn truncates_long_page_text() {
    let json = json!({
        "url": "https://warp.dev/",
        "title": "Long",
        "ready": "complete",
        "text": "a".repeat(20_001),
        "elements": []
    })
    .to_string();

    let output = format_page_snapshot(&json).unwrap();

    assert!(output.ends_with("\n(page text truncated)"), "{output}");
}

#[test]
fn rejects_unexpected_json() {
    let output = format_page_snapshot("null");

    assert!(output.is_err());
}

#[test]
fn type_script_embeds_text_as_a_json_string_literal() {
    let script = type_script(3, "it's \"quoted\"\n</script>", false);

    assert!(
        script.contains(r#"const text = "it's \"quoted\"\n</script>";"#),
        "{script}"
    );
}

#[test]
fn click_script_moves_the_agent_cursor_before_clicking() {
    let script = click_script(4);

    let cursor = script.find("warpPointAt(el);").unwrap();
    let click = script.find("el.click();").unwrap();
    assert!(cursor < click, "{script}");
    assert!(script.contains("}, 400);"), "{script}");
}

#[test]
fn formats_console_messages_by_level() {
    let json = json!([
        {"level": "error", "message": "TypeError: x is undefined"},
        {"level": "network", "message": "404 http://localhost:5173/api/user"}
    ])
    .to_string();

    let output = format_console_messages(&json).unwrap();

    assert_eq!(
        output,
        "[error] TypeError: x is undefined\n[network] 404 http://localhost:5173/api/user"
    );
}

#[test]
fn reports_when_there_are_no_console_messages() {
    let output = format_console_messages("[]").unwrap();

    assert_eq!(
        output,
        "No console messages, page errors or failed requests."
    );
}

#[test]
fn reports_when_console_capture_is_missing() {
    let output = format_console_messages("null").unwrap();

    assert_eq!(output, "Console capture is not available on this page.");
}
