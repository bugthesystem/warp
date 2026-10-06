use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::url_input::is_local_host;

/// What agents ask approval for before reading `file:` URLs, since those expose the user's files.
pub const LOCAL_FILES_SITE: &str = "local files";

/// The site an agent needs the user's approval for before it acts on, or opens, `url`. `None`
/// when no approval is needed: local addresses, and pages that are neither websites nor files.
pub fn site_requiring_approval(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    match url.scheme() {
        "http" | "https" => {}
        "file" => return Some(LOCAL_FILES_SITE.to_owned()),
        _ => return None,
    }
    let host = url.host_str()?;
    if is_local_host(host) {
        return None;
    }
    Some(host.strip_prefix("www.").unwrap_or(host).to_owned())
}

/// Sites the user always allows agents to use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedSites {
    sites: BTreeSet<String>,
    /// Whether agents may use any site without asking.
    #[serde(default)]
    auto_approve: bool,
}

impl ApprovedSites {
    /// Reads approved sites saved at `path`. A missing or unreadable file approves nothing.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).map_err(io::Error::other)?;
        fs::write(path, json)
    }

    /// Whether agents may use `site` without asking.
    pub fn allows(&self, site: &str) -> bool {
        self.auto_approve || self.sites.contains(site)
    }

    pub fn auto_approve(&self) -> bool {
        self.auto_approve
    }

    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
    }

    pub fn approve(&mut self, site: String) {
        self.sites.insert(site);
    }
}

#[cfg(test)]
#[path = "sites_tests.rs"]
mod tests;
