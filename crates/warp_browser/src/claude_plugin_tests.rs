use std::path::Path;

use serde_json::Value;

use super::{FILES, TOKEN_ENV, URL_ENV, install_command, write_marketplace};

fn file(path: &str) -> &'static str {
    FILES
        .iter()
        .find(|(file_path, _)| *file_path == path)
        .map(|(_, contents)| *contents)
        .unwrap()
}

#[test]
fn json_files_parse() {
    for (path, contents) in FILES.iter().filter(|(path, _)| path.ends_with(".json")) {
        assert!(
            serde_json::from_str::<Value>(contents).is_ok(),
            "{path} is not valid JSON"
        );
    }
}

#[test]
fn mcp_server_reads_the_url_and_token_from_warp_variables() {
    let config: Value = serde_json::from_str(file("warp-browser/.mcp.json")).unwrap();
    let server = &config["mcpServers"]["warp-browser"];

    assert_eq!(server["type"], "http");
    assert_eq!(server["url"], format!("${{{URL_ENV}}}"));
    assert_eq!(
        server["headers"]["Authorization"],
        format!("Bearer ${{{TOKEN_ENV}}}")
    );
}

#[test]
fn marketplace_lists_the_plugin_where_it_is_written() {
    let marketplace: Value = serde_json::from_str(file(".claude-plugin/marketplace.json")).unwrap();
    let plugin = &marketplace["plugins"][0];

    assert_eq!(plugin["name"], "warp-browser");
    assert_eq!(plugin["source"], "./warp-browser");
}

#[test]
fn skill_has_a_name_and_description() {
    let skill = file("warp-browser/skills/warp-browser/SKILL.md");

    assert!(
        skill.starts_with("---\nname: warp-browser\ndescription: "),
        "{skill}"
    );
}

#[test]
fn install_command_quotes_the_directory() {
    let command = install_command(Path::new("/Users/me/Warp's Config/claude-plugin"));

    assert_eq!(
        command,
        r"claude plugin marketplace add '/Users/me/Warp'\''s Config/claude-plugin' && claude plugin install warp-browser@warp-browser"
    );
}

#[test]
fn writes_every_file() {
    let dir = tempfile::tempdir().unwrap();

    write_marketplace(dir.path()).unwrap();

    for (path, contents) in FILES {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(path)).unwrap(),
            *contents
        );
    }
}
