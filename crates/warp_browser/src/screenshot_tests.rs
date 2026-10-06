use super::preview_script;

#[test]
fn preview_embeds_the_image_and_escapes_the_file_name() {
    let script = preview_script(b"png", "shot \"1\".png");

    assert!(
        script.contains(r#"image.src = "data:image/png;base64,cG5n";"#),
        "{script}"
    );
    assert!(
        script.contains(r#"name.textContent = "shot \"1\".png";"#),
        "{script}"
    );
}

#[test]
fn preview_buttons_post_page_actions() {
    let script = preview_script(b"png", "shot.png");

    for action in ["screenshot-copy", "screenshot-send", "screenshot-reveal"] {
        assert!(
            script.contains(&format!(r#""{action}""#)),
            "{action}: {script}"
        );
    }
    assert!(
        script.contains(r#"window.ipc.postMessage("warp:action:" + action)"#),
        "{script}"
    );
}
