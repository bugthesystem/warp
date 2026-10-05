use warpui::{AppContext, Entity, SingletonEntity, ViewHandle, WeakViewHandle, WindowId};

use super::BrowserView;

/// Tracks open [`BrowserView`]s so their web views can follow their panes after every frame, and
/// so agents can address them by a stable tab id.
#[derive(Default)]
pub struct BrowserViewRegistry {
    views: Vec<RegisteredView>,
    next_tab_id: u64,
    current_tab_id: Option<u64>,
}

struct RegisteredView {
    tab_id: u64,
    view: WeakViewHandle<BrowserView>,
}

impl BrowserViewRegistry {
    /// Registers a view, makes it the current tab and returns its tab id.
    pub fn register(&mut self, view: WeakViewHandle<BrowserView>) -> u64 {
        self.next_tab_id += 1;
        let tab_id = self.next_tab_id;
        self.views.push(RegisteredView { tab_id, view });
        self.current_tab_id = Some(tab_id);
        tab_id
    }

    /// Open browser views with their tab ids, oldest first.
    pub fn tabs(&self, ctx: &AppContext) -> Vec<(u64, ViewHandle<BrowserView>)> {
        self.views
            .iter()
            .filter_map(|registered| {
                registered
                    .view
                    .upgrade(ctx)
                    .map(|view| (registered.tab_id, view))
            })
            .collect()
    }

    /// The tab with `tab_id`, or the current tab when `tab_id` is `None`. Falls back to the most
    /// recently opened tab when the current one was closed.
    pub fn resolve(
        &self,
        tab_id: Option<u64>,
        ctx: &AppContext,
    ) -> Option<(u64, ViewHandle<BrowserView>)> {
        let mut tabs = self.tabs(ctx);
        let wanted = tab_id.or(self.current_tab_id);
        match tabs.iter().position(|(id, _)| Some(*id) == wanted) {
            Some(index) => Some(tabs.swap_remove(index)),
            None if tab_id.is_some() => None,
            None => tabs.pop(),
        }
    }

    pub fn current_tab_id(&self) -> Option<u64> {
        self.current_tab_id
    }

    pub fn set_current_tab(&mut self, tab_id: u64) {
        self.current_tab_id = Some(tab_id);
    }
}

impl Entity for BrowserViewRegistry {
    type Event = ();
}

impl SingletonEntity for BrowserViewRegistry {}

/// Aligns the web view of every browser view in `window_id` with its pane as drawn in the frame
/// that just finished.
pub fn sync_webviews(window_id: WindowId, ctx: &mut AppContext) {
    let views = BrowserViewRegistry::handle(ctx).update(ctx, |registry, ctx| {
        registry
            .views
            .retain(|registered| registered.view.upgrade(ctx).is_some());
        registry
            .views
            .iter()
            .filter(|registered| registered.view.window_id(ctx) == Some(window_id))
            .filter_map(|registered| registered.view.upgrade(ctx))
            .collect::<Vec<_>>()
    });
    for view in views {
        view.update(ctx, |view, ctx| view.sync_webview(ctx));
    }
}
