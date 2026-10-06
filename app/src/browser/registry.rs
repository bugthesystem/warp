use warpui::{AppContext, Entity, SingletonEntity, ViewHandle, WeakViewHandle, WindowId};

use super::BrowserView;

/// Tracks open [`BrowserView`]s so their web views can follow their panes after every frame, and
/// hands out the tab ids agents use to address browser tabs.
#[derive(Default)]
pub struct BrowserViewRegistry {
    views: Vec<WeakViewHandle<BrowserView>>,
    last_tab_id: u64,
    current_tab_id: Option<u64>,
}

impl BrowserViewRegistry {
    pub fn register(&mut self, view: WeakViewHandle<BrowserView>) {
        self.views.push(view);
    }

    pub fn new_tab_id(&mut self) -> u64 {
        self.last_tab_id += 1;
        self.last_tab_id
    }

    /// Open browser tabs and the view holding each, in the order the views were opened.
    pub fn tabs(&self, ctx: &AppContext) -> Vec<(u64, ViewHandle<BrowserView>)> {
        self.views
            .iter()
            .filter_map(|view| view.upgrade(ctx))
            .filter(|view| view.as_ref(ctx).is_attached())
            .flat_map(|view| {
                view.as_ref(ctx)
                    .tab_ids()
                    .map(|tab_id| (tab_id, view.clone()))
                    .collect::<Vec<_>>()
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
            None => tabs.into_iter().max_by_key(|(id, _)| *id),
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

/// Aligns the web views of every browser view in `window_id` with its pane as drawn in the frame
/// that just finished.
pub fn sync_webviews(window_id: WindowId, ctx: &mut AppContext) {
    let views = BrowserViewRegistry::handle(ctx).update(ctx, |registry, ctx| {
        registry.views.retain(|view| view.upgrade(ctx).is_some());
        registry
            .views
            .iter()
            .filter(|view| view.window_id(ctx) == Some(window_id))
            .filter_map(|view| view.upgrade(ctx))
            .collect::<Vec<_>>()
    });
    for view in views {
        view.update(ctx, |view, ctx| view.sync_webview(ctx));
    }
}
