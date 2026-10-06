use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::Frame;
use crate::browser::find_chromium;

#[test]
fn formats_console_messages() {
    let params = json!({
        "type": "warn",
        "args": [
            {"type": "string", "value": "slow frame"},
            {"type": "number", "value": 42},
            {"type": "object", "description": "Object"}
        ]
    });
    assert_eq!(
        log_line("Runtime.consoleAPICalled", &params),
        Some("console.warn: slow frame 42 Object".to_owned())
    );
}

#[test]
fn formats_uncaught_errors_and_log_entries() {
    let error = json!({
        "exceptionDetails": {
            "text": "Uncaught",
            "exception": {"description": "TypeError: x is undefined"}
        }
    });
    assert_eq!(
        log_line("Runtime.exceptionThrown", &error),
        Some("error: TypeError: x is undefined".to_owned())
    );

    let entry = json!({
        "entry": {"level": "error", "text": "Failed to load resource", "url": "http://localhost/a.js"}
    });
    assert_eq!(
        log_line("Log.entryAdded", &entry),
        Some("error: Failed to load resource (http://localhost/a.js)".to_owned())
    );
}

#[test]
fn ignores_other_events_and_truncates_long_lines() {
    assert_eq!(log_line("Page.loadEventFired", &json!({})), None);

    let long = "x".repeat(MAX_LOG_LINE * 2);
    let params = json!({"type": "log", "args": [{"type": "string", "value": long}]});
    let line = log_line("Runtime.consoleAPICalled", &params).expect("a line");
    assert_eq!(line.chars().count(), MAX_LOG_LINE);
}

/// Streams a real page. Needs a Chromium-based browser, so it only runs when asked for:
/// `WARP_PREVIEW_CHROMIUM=/path/to/chrome cargo nextest run -p warp_preview --run-ignored all`.
#[test]
#[ignore = "starts a real browser"]
fn streams_frames_title_and_console_from_a_page() {
    let chromium = find_chromium(None).expect("a Chromium-based browser");
    let page = "data:text/html,<title>Preview test</title><body style='background:%23c33'>\
                <script>console.log('hello from the page')</script>";
    let (events_tx, events_rx) = async_channel::unbounded();
    let stream = start_screencast(chromium, page.to_owned(), (640, 400), Rate::Full, events_tx)
        .expect("starts");

    let (mut frame, mut title, mut log) = (None, None, None);
    let deadline = Instant::now() + Duration::from_secs(30);
    let done = |frame: &Option<Frame>, title: &Option<String>, log: &Option<String>| {
        frame.is_some() && title.as_deref() == Some("Preview test") && log.is_some()
    };
    while !done(&frame, &title, &log) && Instant::now() < deadline {
        let Ok(event) = events_rx.try_recv() else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        match event {
            StreamEvent::Frame(got) => frame = Some(got),
            StreamEvent::Title(got) => title = Some(got),
            StreamEvent::Log(got) => log = Some(got),
            StreamEvent::Ended(reason) => panic!("stream ended: {reason}"),
            StreamEvent::Url(_) | StreamEvent::Minimized(_) => {}
        }
    }
    let frame = frame.expect("a frame");
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(title.as_deref(), Some("Preview test"));
    assert_eq!(log.as_deref(), Some("console.log: hello from the page"));
    drop(stream);
}
