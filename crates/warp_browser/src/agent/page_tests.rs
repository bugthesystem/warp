use serde_json::json;

use super::{
    PagePoint, format_console_messages, format_page_snapshot, page_point, point_script,
    prepare_type_script, type_now_script,
};

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
fn type_now_script_embeds_text_as_a_json_string_literal() {
    let script = type_now_script(3, "it's \"quoted\"\n</script>", false);

    assert!(
        script.contains(r#"const text = "it's \"quoted\"\n</script>";"#),
        "{script}"
    );
}

#[test]
fn point_script_glides_the_cursor_and_marks_agent_input_without_clicking() {
    let script = point_script(4);

    assert!(
        script.contains("const point = warpPointAt(el);"),
        "{script}"
    );
    assert!(
        script.contains("window.__warpAgentInputUntil = Date.now() + 2150;"),
        "{script}"
    );
    assert!(
        script.contains("return { ok: true, x: point.x, y: point.y };"),
        "{script}"
    );
    assert!(!script.contains(".click()"), "{script}");
}

#[test]
fn prepare_type_script_selects_the_value_to_replace() {
    let script = prepare_type_script(2);

    assert!(script.contains("el.focus();"), "{script}");
    assert!(
        script.contains("else if (el.select) el.select();"),
        "{script}"
    );
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

#[test]
fn reads_the_point_the_cursor_reached() {
    let point = page_point(r#"{"ok":true,"x":120.5,"y":48}"#);

    assert_eq!(point, Ok(PagePoint { x: 120.5, y: 48. }));
}

#[test]
fn reports_the_page_error_instead_of_a_point() {
    let point = page_point(
        r#"{"ok":false,"error":"No element 9; call browser_read to refresh the element numbers."}"#,
    );

    assert_eq!(
        point,
        Err("No element 9; call browser_read to refresh the element numbers.".to_owned())
    );
}
