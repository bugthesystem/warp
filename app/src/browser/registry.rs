use warpui::{AppContext, Entity, SingletonEntity, WeakViewHandle, WindowId};

use super::BrowserView;

/// Tracks open [`BrowserView`]s so their web views can follow their panes after every frame.
#[derive(Default)]
pub struct BrowserViewRegistry {
    views: Vec<WeakViewHandle<BrowserView>>,
}

impl BrowserViewRegistry {
    pub fn register(&mut self, view: WeakViewHandle<BrowserView>) {
        self.views.push(view);
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
