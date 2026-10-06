use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::is_local_address;

/// Most visits kept; older ones are dropped.
const MAX_ENTRIES: usize = 200;

/// A visited page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: Option<String>,
    /// Seconds since the Unix epoch.
    pub visited_at: u64,
    /// Whether an agent, rather than the user, opened the page.
    #[serde(default)]
    pub opened_by_agent: bool,
}

/// History split the way the new-tab page shows it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HistorySections<'a> {
    /// Pages on local addresses, such as dev servers.
    pub local_apps: Vec<&'a HistoryEntry>,
    /// Other pages an agent opened.
    pub opened_by_agents: Vec<&'a HistoryEntry>,
    /// Everything else.
    pub recent: Vec<&'a HistoryEntry>,
}

/// Pages visited in browser panes, most recent first, with one entry per URL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserHistory {
    entries: Vec<HistoryEntry>,
}

impl BrowserHistory {
    /// Reads history saved at `path`. A missing or unreadable file gives empty history, since
    /// history is a convenience.
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

    /// Records a visit to `url`, moving an earlier visit to the same URL to the front and keeping
    /// its title. Only web pages are recorded.
    pub fn record_visit(&mut self, url: &str, visited_at: u64, opened_by_agent: bool) {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return;
        }
        let title = self
            .entries
            .iter()
            .position(|entry| entry.url == url)
            .and_then(|index| self.entries.remove(index).title);
        self.entries.insert(
            0,
            HistoryEntry {
                url: url.to_owned(),
                title,
                visited_at,
                opened_by_agent,
            },
        );
        self.entries.truncate(MAX_ENTRIES);
    }

    /// Sets the title of the most recent visit to `url`, if it was recorded.
    pub fn set_title(&mut self, url: &str, title: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.url == url) {
            entry.title = (!title.is_empty()).then(|| title.to_owned());
        }
    }

    /// Visits, most recent first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// The most recent visits in each section, at most `limit` per section.
    pub fn sections(&self, limit: usize) -> HistorySections<'_> {
        let mut sections = HistorySections::default();
        for entry in &self.entries {
            let section = if is_local_address(&entry.url) {
                &mut sections.local_apps
            } else if entry.opened_by_agent {
                &mut sections.opened_by_agents
            } else {
                &mut sections.recent
            };
            if section.len() < limit {
                section.push(entry);
            }
        }
        sections
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
