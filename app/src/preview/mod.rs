//! The preview pane: live previews of other programs, drawn by Warp from frames their streams
//! send, so the same stage serves a pane and the picture-in-picture window.

mod agent;
mod pip;
mod stage;
mod streams;
mod view;

use warpui::{
    AppContext, Entity, SingletonEntity, ViewContext, ViewHandle, WeakViewHandle, WindowId,
};

pub use agent::PreviewAgent;
pub use pip::PreviewPipView;
pub use streams::{PreviewId, PreviewStreams};
pub use view::{PreviewView, PreviewViewEvent};

/// Tracks the views showing previews, so their previews' rates follow what was drawn, and holds
/// each window's picture-in-picture.
#[derive(Default)]
pub struct PreviewRegistry {
    views: Vec<WeakViewHandle<PreviewView>>,
    pictures_in_picture: Vec<(WindowId, ViewHandle<PreviewPipView>)>,
}

impl PreviewRegistry {
    pub fn register_view(&mut self, view: WeakViewHandle<PreviewView>) {
        self.views.push(view);
    }

    /// Open preview panes, in the order they were opened.
    pub fn views(&self, ctx: &AppContext) -> Vec<ViewHandle<PreviewView>> {
        self.views
            .iter()
            .filter_map(|view| view.upgrade(ctx))
            .filter(|view| view.as_ref(ctx).is_attached())
            .collect()
    }

    pub fn picture_in_picture(&self, window_id: WindowId) -> Option<&ViewHandle<PreviewPipView>> {
        self.pictures_in_picture
            .iter()
            .find(|(window, _)| *window == window_id)
            .map(|(_, view)| view)
    }

    fn set_picture_in_picture(
        &mut self,
        window_id: WindowId,
        view: Option<ViewHandle<PreviewPipView>>,
        ctx: &mut warpui::ModelContext<Self>,
    ) {
        self.pictures_in_picture
            .retain(|(window, _)| *window != window_id);
        if let Some(view) = view {
            self.pictures_in_picture.push((window_id, view));
        }
        ctx.notify();
    }
}

impl Entity for PreviewRegistry {
    type Event = ();
}

impl SingletonEntity for PreviewRegistry {}

/// Shows `previews` with `front` large in the window's picture-in-picture, adding them to the one
/// already there.
pub fn open_picture_in_picture<V: warpui::View>(
    previews: Vec<PreviewId>,
    front: PreviewId,
    ctx: &mut ViewContext<V>,
) {
    let window_id = ctx.window_id();
    if let Some(existing) = PreviewRegistry::as_ref(ctx)
        .picture_in_picture(window_id)
        .cloned()
    {
        existing.update(ctx, |pip, ctx| pip.add_previews(previews, front, ctx));
        return;
    }
    let pip = ctx.add_typed_action_view(|ctx| PreviewPipView::new(previews, front, ctx));
    PreviewRegistry::handle(ctx).update(ctx, |registry, ctx| {
        registry.set_picture_in_picture(window_id, Some(pip), ctx)
    });
}

/// Removes the window's picture-in-picture without stopping its previews.
fn close_picture_in_picture<V: warpui::View>(ctx: &mut ViewContext<V>) {
    let window_id = ctx.window_id();
    PreviewRegistry::handle(ctx).update(ctx, |registry, ctx| {
        registry.set_picture_in_picture(window_id, None, ctx)
    });
}

/// Sets the rate of every preview shown in `window_id` from the frame that just finished.
pub fn sync_previews(window_id: WindowId, ctx: &mut AppContext) {
    let (views, pip) = PreviewRegistry::handle(ctx).update(ctx, |registry, ctx| {
        registry.views.retain(|view| view.upgrade(ctx).is_some());
        let views = registry
            .views
            .iter()
            .filter(|view| view.window_id(ctx) == Some(window_id))
            .filter_map(|view| view.upgrade(ctx))
            .collect::<Vec<_>>();
        (views, registry.picture_in_picture(window_id).cloned())
    });
    for view in views {
        view.update(ctx, |view, ctx| view.sync_rates(ctx));
    }
    if let Some(pip) = pip {
        pip.update(ctx, |pip, ctx| pip.sync_rates(ctx));
    }
}

/// Whether preview panes are available.
pub fn is_enabled() -> bool {
    crate::features::FeatureFlag::PreviewPane.is_enabled() && cfg!(not(target_family = "wasm"))
}
