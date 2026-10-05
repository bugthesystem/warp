use serde_json::json;

use super::{format_page_snapshot, type_script};

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
