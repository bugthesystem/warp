use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use warp_browser::history::{BrowserHistory, HistorySections};
use warpui::{Entity, ModelContext, SingletonEntity};

const HISTORY_FILE_NAME: &str = "browser-history.json";

/// Pages visited in browser panes, kept in Warp's home config directory for the new-tab page.
pub struct BrowserHistoryModel {
    history: BrowserHistory,
    path: Option<PathBuf>,
}

impl BrowserHistoryModel {
    pub fn new() -> Self {
        let path = warp_core::paths::warp_home_config_dir().map(|dir| dir.join(HISTORY_FILE_NAME));
        let history = path
            .as_deref()
            .map(BrowserHistory::load)
            .unwrap_or_default();
        Self { history, path }
    }

    pub fn sections(&self, limit: usize) -> HistorySections<'_> {
        self.history.sections(limit)
    }

    pub fn record_visit(&mut self, url: &str, opened_by_agent: bool, ctx: &mut ModelContext<Self>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        self.history.record_visit(url, now, opened_by_agent);
        self.save();
        ctx.notify();
    }

    pub fn set_title(&mut self, url: &str, title: &str, ctx: &mut ModelContext<Self>) {
        self.history.set_title(url, title);
        self.save();
        ctx.notify();
    }

    fn save(&self) {
        if let Some(path) = &self.path
            && let Err(err) = self.history.save(path)
        {
            log::warn!("Failed to save browser history: {err:#}");
        }
    }
}

impl Entity for BrowserHistoryModel {
    type Event = ();
}

impl SingletonEntity for BrowserHistoryModel {}
