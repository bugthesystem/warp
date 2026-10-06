use super::{display_url, is_local_address, resolve_input};

#[test]
fn urls_with_known_schemes_load_as_typed() {
    assert_eq!(
        resolve_input("https://warp.dev/docs"),
        "https://warp.dev/docs"
    );
    assert_eq!(
        resolve_input("http://localhost:5173/"),
        "http://localhost:5173/"
    );
    assert_eq!(resolve_input("about:blank"), "about:blank");
    assert_eq!(
        resolve_input("file:///tmp/index.html"),
        "file:///tmp/index.html"
    );
}

#[test]
fn surrounding_whitespace_is_ignored() {
    assert_eq!(resolve_input("  https://warp.dev  "), "https://warp.dev");
}

#[test]
fn empty_input_loads_a_blank_page() {
    assert_eq!(resolve_input("   "), "about:blank");
}

#[test]
fn local_hosts_load_over_http() {
    assert_eq!(resolve_input("localhost"), "http://localhost");
    assert_eq!(resolve_input("localhost:3000"), "http://localhost:3000");
    assert_eq!(
        resolve_input("localhost:3000/app?x=1"),
        "http://localhost:3000/app?x=1"
    );
    assert_eq!(resolve_input("127.0.0.1:8080"), "http://127.0.0.1:8080");
    assert_eq!(resolve_input("[::1]:8080"), "http://[::1]:8080");
    assert_eq!(
        resolve_input("app.localhost:4000"),
        "http://app.localhost:4000"
    );
}

#[test]
fn other_hosts_load_over_https() {
    assert_eq!(resolve_input("warp.dev"), "https://warp.dev");
    assert_eq!(
        resolve_input("docs.rs/wry/latest"),
        "https://docs.rs/wry/latest"
    );
    assert_eq!(
        resolve_input("example.com:8443"),
        "https://example.com:8443"
    );
}

#[test]
fn everything_else_becomes_a_search() {
    assert_eq!(
        resolve_input("rust wry webview"),
        "https://www.google.com/search?q=rust+wry+webview"
    );
    assert_eq!(resolve_input("wry"), "https://www.google.com/search?q=wry");
    assert_eq!(
        resolve_input("a&b=c"),
        "https://www.google.com/search?q=a%26b%3Dc"
    );
    assert_eq!(
        resolve_input("foo:bar"),
        "https://www.google.com/search?q=foo%3Abar"
    );
    assert_eq!(
        resolve_input(".com"),
        "https://www.google.com/search?q=.com"
    );
}

#[test]
fn local_addresses_are_recognized_with_or_without_a_scheme() {
    assert!(is_local_address("http://localhost:5173/app"));
    assert!(is_local_address("localhost:3000"));
    assert!(is_local_address("http://127.0.0.1:8080"));
    assert!(is_local_address("http://[::1]:8080/"));
    assert!(is_local_address("http://app.localhost:4000"));
}

#[test]
fn remote_addresses_and_searches_are_not_local() {
    assert!(!is_local_address("https://warp.dev"));
    assert!(!is_local_address("localhost.example.com"));
    assert!(!is_local_address("run localhost"));
}

#[test]
fn display_url_drops_scheme_www_and_bare_trailing_slash() {
    assert_eq!(display_url("https://www.google.com/"), "google.com");
    assert_eq!(display_url("http://localhost:5173/"), "localhost:5173");
}

#[test]
fn display_url_keeps_paths_and_queries() {
    assert_eq!(
        display_url("https://docs.rs/wry/latest/"),
        "docs.rs/wry/latest/"
    );
    assert_eq!(
        display_url("https://www.google.com/search?q=wry"),
        "google.com/search?q=wry"
    );
}

#[test]
fn display_url_shows_other_schemes_as_they_are() {
    assert_eq!(display_url("about:blank"), "about:blank");
    assert_eq!(
        display_url("file:///tmp/index.html"),
        "file:///tmp/index.html"
    );
}
