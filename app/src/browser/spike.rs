//! Attaches a web view at a fixed rect in a window, to check that native web view embedding works
//! before the browser pane exists.

use std::collections::HashMap;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warp_browser::WebView;
use warpui::{Entity, ModelContext, SingletonEntity, WindowId};

const SPIKE_URL: &str = "https://example.com";

#[derive(Default)]
pub struct BrowserSpike {
    webviews: HashMap<WindowId, WebView>,
}

impl BrowserSpike {
    /// Shows a web view in the window, or removes it if one is already showing.
    pub fn toggle(&mut self, window_id: WindowId, ctx: &mut ModelContext<Self>) {
        if self.webviews.remove(&window_id).is_some() {
            return;
        }

        let Some(parent) = warp_browser::window_parent(ctx, window_id) else {
            log::warn!("Browser spike: window {window_id:?} has no native view");
            return;
        };
        let bounds = RectF::new(vec2f(80., 120.), vec2f(900., 600.));
        match WebView::new(&parent, SPIKE_URL, bounds, |event| {
            log::info!("Browser spike: {event:?}");
        }) {
            Ok(webview) => {
                self.webviews.insert(window_id, webview);
            }
            Err(err) => log::error!("Browser spike: failed to create web view: {err}"),
        }
    }
}

impl Entity for BrowserSpike {
    type Event = ();
}

impl SingletonEntity for BrowserSpike {}
