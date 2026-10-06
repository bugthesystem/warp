use std::fs;

use super::load_or_create_token;

#[test]
fn creates_a_token_when_the_file_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("browser-mcp-token");

    let token = load_or_create_token(&path).unwrap();

    assert_eq!(token.len(), 32);
    assert_eq!(fs::read_to_string(&path).unwrap(), token);
}

#[test]
fn reuses_the_stored_token() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-mcp-token");
    fs::write(&path, "stored-token\n").unwrap();

    let token = load_or_create_token(&path).unwrap();

    assert_eq!(token, "stored-token");
}

#[test]
fn replaces_an_empty_token_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-mcp-token");
    fs::write(&path, "  \n").unwrap();

    let token = load_or_create_token(&path).unwrap();

    assert_eq!(token.len(), 32);
}

#[cfg(unix)]
#[test]
fn token_file_is_readable_only_by_its_owner() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-mcp-token");

    load_or_create_token(&path).unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
