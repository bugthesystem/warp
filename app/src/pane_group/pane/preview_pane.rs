use warpui::{AppContext, ModelHandle, View, ViewContext, ViewHandle};

use super::view::PaneView;
use super::{
    DetachType, PaneConfiguration, PaneContent, PaneGroup, PaneId, ShareableLink,
    ShareableLinkError,
};
use crate::app_state::LeafContents;
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::preview::{PreviewId, PreviewView, PreviewViewEvent};

pub struct PreviewPane {
    view: ViewHandle<PaneView<PreviewView>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
}

impl PreviewPane {
    /// Creates a preview pane showing `previews`, or its start page when there are none.
    pub fn new<V: View>(previews: Vec<PreviewId>, ctx: &mut ViewContext<V>) -> Self {
        let preview_view = ctx.add_typed_action_view(|ctx| PreviewView::new(previews, ctx));
        let pane_configuration = preview_view.as_ref(ctx).pane_configuration();
        let view = ctx.add_typed_action_view(|ctx| {
            let pane_id = PaneId::from_preview_pane_ctx(ctx);
            PaneView::new(pane_id, preview_view, (), pane_configuration.clone(), ctx)
        });
        Self {
            view,
            pane_configuration,
        }
    }

    pub fn preview_view(&self, ctx: &AppContext) -> ViewHandle<PreviewView> {
        self.view.as_ref(ctx).child(ctx)
    }
}

impl PaneContent for PreviewPane {
    fn id(&self) -> PaneId {
        PaneId::from_preview_pane_view(&self.view)
    }

    fn attach(
        &self,
        _group: &PaneGroup,
        focus_handle: PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));

        let preview_view = self.preview_view(ctx);
        preview_view.update(ctx, |view, ctx| view.set_attached(true, ctx));
        let pane_id = self.id();

        ctx.subscribe_to_view(
            &preview_view,
            move |pane_group, _, event, ctx| match event {
                PreviewViewEvent::Pane(pane_event) => {
                    pane_group.handle_pane_event(pane_id, pane_event, ctx)
                }
            },
        );
        ctx.subscribe_to_view(&self.view, move |group, _, event, ctx| {
            group.handle_pane_view_event(pane_id, event, ctx);
        });
    }

    fn detach(
        &self,
        _group: &PaneGroup,
        detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        let preview_view = self.preview_view(ctx);
        match detach_type {
            DetachType::Closed => preview_view.update(ctx, |view, ctx| {
                view.set_attached(false, ctx);
                view.close_previews(ctx);
            }),
            DetachType::HiddenForClose => {
                preview_view.update(ctx, |view, ctx| view.set_attached(false, ctx));
            }
            DetachType::Moved => {}
        }
        ctx.unsubscribe_to_view(&preview_view);
        ctx.unsubscribe_to_view(&self.view);
    }

    fn snapshot(&self, _app: &AppContext) -> LeafContents {
        LeafContents::Preview
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.preview_view(ctx)
            .update(ctx, |view, ctx| view.focus(ctx));
    }

    fn shareable_link(
        &self,
        _ctx: &mut ViewContext<PaneGroup>,
    ) -> Result<ShareableLink, ShareableLinkError> {
        Ok(ShareableLink::Base)
    }

    fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    fn is_pane_being_dragged(&self, ctx: &AppContext) -> bool {
        self.view.as_ref(ctx).is_being_dragged()
    }
}
