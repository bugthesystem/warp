//! The Claude Code plugin that gives Claude Code started in a Warp terminal the browser tools,
//! with no token to copy: its MCP server reads the address and token from variables Warp sets in
//! every terminal it starts.

use std::path::Path;

/// The variable holding the browser MCP endpoint's URL in Warp terminals.
pub const URL_ENV: &str = "WARP_BROWSER_MCP_URL";
/// The variable holding the browser MCP endpoint's bearer token in Warp terminals.
pub const TOKEN_ENV: &str = "WARP_BROWSER_MCP_TOKEN";

const MARKETPLACE_NAME: &str = "warp-browser";
const PLUGIN_NAME: &str = "warp-browser";

/// The plugin's marketplace, as paths relative to its root directory and their contents.
pub const FILES: &[(&str, &str)] = &[
    (
        ".claude-plugin/marketplace.json",
        include_str!("../claude-plugin/.claude-plugin/marketplace.json"),
    ),
    (
        "warp-browser/.claude-plugin/plugin.json",
        include_str!("../claude-plugin/warp-browser/.claude-plugin/plugin.json"),
    ),
    (
        "warp-browser/.mcp.json",
        include_str!("../claude-plugin/warp-browser/.mcp.json"),
    ),
    (
        "warp-browser/skills/warp-browser/SKILL.md",
        include_str!("../claude-plugin/warp-browser/skills/warp-browser/SKILL.md"),
    ),
];

/// Writes the marketplace into `dir`, replacing an earlier copy.
pub fn write_marketplace(dir: &Path) -> std::io::Result<()> {
    for (path, contents) in FILES {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}

/// A shell command that installs the plugin from the marketplace written to `dir`.
pub fn install_command(dir: &Path) -> String {
    let dir = shell_quote(&dir.to_string_lossy());
    format!(
        "claude plugin marketplace add {dir} && claude plugin install {PLUGIN_NAME}@{MARKETPLACE_NAME}"
    )
}

/// Quotes `value` for POSIX shells.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
#[path = "claude_plugin_tests.rs"]
mod tests;
