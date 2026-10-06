use super::*;

const VERSIONS: &str = r#"{
  "timestamp": "2026-10-01T00:00:00.000Z",
  "channels": {
    "Stable": {
      "channel": "Stable",
      "version": "141.0.7390.54",
      "revision": "1509326",
      "downloads": {
        "chrome": [
          {"platform": "linux64", "url": "https://example.com/chrome-linux64.zip"}
        ],
        "chrome-headless-shell": [
          {"platform": "linux64", "url": "https://example.com/shell-linux64.zip"},
          {"platform": "mac-arm64", "url": "https://example.com/shell-mac-arm64.zip"}
        ]
      }
    },
    "Beta": {"channel": "Beta", "version": "142.0.0.0", "revision": "1", "downloads": {}}
  }
}"#;

#[test]
fn picks_the_stable_headless_shell_for_the_platform() {
    assert_eq!(
        parse_known_good_versions(VERSIONS, "mac-arm64"),
        Some(ChromiumDownload {
            version: "141.0.7390.54".to_owned(),
            url: "https://example.com/shell-mac-arm64.zip".to_owned(),
        })
    );
}

#[test]
fn has_nothing_for_a_platform_without_builds() {
    assert_eq!(parse_known_good_versions(VERSIONS, "win64"), None);
    assert_eq!(parse_known_good_versions("not json", "linux64"), None);
}

#[test]
fn finds_an_installed_build_by_its_version_file() {
    let Some(platform) = platform_name() else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(installed_chromium(dir.path()), None);

    let binary = binary_path(&dir.path().join("141.0.1"), platform);
    fs::create_dir_all(binary.parent().expect("has a parent")).expect("creates dirs");
    fs::write(&binary, b"").expect("writes binary");
    fs::write(dir.path().join(VERSION_FILE), "141.0.1\n").expect("writes version");

    assert_eq!(installed_chromium(dir.path()), Some(binary));
}
