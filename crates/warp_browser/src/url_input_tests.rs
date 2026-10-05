use super::resolve_input;

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
