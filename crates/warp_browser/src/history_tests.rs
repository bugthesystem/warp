use super::{BrowserHistory, HistoryEntry};

fn urls(history: &BrowserHistory) -> Vec<&str> {
    history
        .entries()
        .iter()
        .map(|entry| entry.url.as_str())
        .collect()
}

#[test]
fn records_visits_most_recent_first() {
    let mut history = BrowserHistory::default();

    history.record_visit("https://warp.dev/", 1, false);
    history.record_visit("http://localhost:5173/", 2, false);

    assert_eq!(
        urls(&history),
        ["http://localhost:5173/", "https://warp.dev/"]
    );
}

#[test]
fn revisiting_moves_the_entry_to_the_front_and_keeps_its_title() {
    let mut history = BrowserHistory::default();
    history.record_visit("https://warp.dev/", 1, false);
    history.set_title("https://warp.dev/", "Warp");
    history.record_visit("https://docs.rs/", 2, false);

    history.record_visit("https://warp.dev/", 3, true);

    assert_eq!(
        history.entries()[0],
        HistoryEntry {
            url: "https://warp.dev/".to_owned(),
            title: Some("Warp".to_owned()),
            visited_at: 3,
            opened_by_agent: true,
        }
    );
    assert_eq!(history.entries().len(), 2);
}

#[test]
fn ignores_pages_that_are_not_websites() {
    let mut history = BrowserHistory::default();

    history.record_visit("about:blank", 1, false);
    history.record_visit("file:///tmp/index.html", 2, false);

    assert!(history.entries().is_empty());
}

#[test]
fn keeps_at_most_two_hundred_visits() {
    let mut history = BrowserHistory::default();

    for visit in 0..205 {
        history.record_visit(&format!("https://example.com/{visit}"), visit, false);
    }

    assert_eq!(history.entries().len(), 200);
    assert_eq!(history.entries()[0].url, "https://example.com/204");
}

#[test]
fn saves_and_loads_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("browser-history.json");
    let mut history = BrowserHistory::default();
    history.record_visit("https://warp.dev/", 1, false);

    history.save(&path).unwrap();

    assert_eq!(BrowserHistory::load(&path), history);
}

#[test]
fn missing_or_corrupt_history_loads_empty() {
    let dir = tempfile::tempdir().unwrap();
    let corrupt = dir.path().join("corrupt.json");
    std::fs::write(&corrupt, "not json").unwrap();

    assert!(
        BrowserHistory::load(&dir.path().join("missing.json"))
            .entries()
            .is_empty()
    );
    assert!(BrowserHistory::load(&corrupt).entries().is_empty());
}

#[test]
fn sections_split_local_apps_agent_pages_and_recent_pages() {
    let mut history = BrowserHistory::default();
    history.record_visit("https://warp.dev/", 1, false);
    history.record_visit("https://github.com/", 2, true);
    history.record_visit("http://localhost:5173/", 3, true);

    let sections = history.sections(5);

    let urls = |entries: &[&HistoryEntry]| -> Vec<String> {
        entries.iter().map(|entry| entry.url.clone()).collect()
    };
    assert_eq!(urls(&sections.local_apps), ["http://localhost:5173/"]);
    assert_eq!(urls(&sections.opened_by_agents), ["https://github.com/"]);
    assert_eq!(urls(&sections.recent), ["https://warp.dev/"]);
}

#[test]
fn sections_are_limited_per_section() {
    let mut history = BrowserHistory::default();
    for visit in 0..4 {
        history.record_visit(&format!("https://example.com/{visit}"), visit, false);
    }

    let sections = history.sections(2);

    assert_eq!(sections.recent.len(), 2);
    assert_eq!(sections.recent[0].url, "https://example.com/3");
}

#[test]
fn history_saved_before_agent_tracking_loads_as_user_visits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-history.json");
    std::fs::write(
        &path,
        r#"{"entries":[{"url":"https://warp.dev/","title":null,"visited_at":1}]}"#,
    )
    .unwrap();

    let history = BrowserHistory::load(&path);

    assert!(!history.entries()[0].opened_by_agent);
}
