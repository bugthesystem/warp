use warpui::{AppContext, ModelHandle, View, ViewContext, ViewHandle};

use super::view::PaneView;
use super::{
    DetachType, PaneConfiguration, PaneContent, PaneGroup, PaneId, ShareableLink,
    ShareableLinkError,
};
use crate::app_state::{BrowserPaneSnapshot, LeafContents};
use crate::browser::{BrowserView, BrowserViewEvent};
use crate::pane_group::focus_state::PaneFocusHandle;

pub struct BrowserPane {
    view: ViewHandle<PaneView<BrowserView>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
}

impl BrowserPane {
    /// Creates a browser pane showing `url`, or a default page when `url` is `None`.
    pub fn new<V: View>(url: Option<String>, ctx: &mut ViewContext<V>) -> Self {
        let browser_view = ctx.add_typed_action_view(|ctx| BrowserView::new(url, ctx));
        Self::from_view(browser_view, ctx)
    }

    /// Recreates a browser pane from a snapshot saved before a restart.
    pub fn restore<V: View>(snapshot: &BrowserPaneSnapshot, ctx: &mut ViewContext<V>) -> Self {
        let browser_view = ctx.add_typed_action_view(|ctx| {
            BrowserView::with_tabs(&snapshot.tab_urls, snapshot.active_tab_index, ctx)
        });
        Self::from_view(browser_view, ctx)
    }

    fn from_view<V: View>(browser_view: ViewHandle<BrowserView>, ctx: &mut ViewContext<V>) -> Self {
        let pane_configuration = browser_view.as_ref(ctx).pane_configuration();

        let view = ctx.add_typed_action_view(|ctx| {
            let pane_id = PaneId::from_browser_pane_ctx(ctx);
            PaneView::new(pane_id, browser_view, (), pane_configuration.clone(), ctx)
        });

        Self {
            view,
            pane_configuration,
        }
    }

    pub fn browser_view(&self, ctx: &AppContext) -> ViewHandle<BrowserView> {
        self.view.as_ref(ctx).child(ctx)
    }
}

impl PaneContent for BrowserPane {
    fn id(&self) -> PaneId {
        PaneId::from_browser_pane_view(&self.view)
    }

    fn attach(
        &self,
        _group: &PaneGroup,
        focus_handle: PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));

        let browser_view = self.browser_view(ctx);
        let pane_id = self.id();

        ctx.subscribe_to_view(&browser_view, move |pane_group, _, event, ctx| {
            let BrowserViewEvent::Pane(pane_event) = event;
            pane_group.handle_pane_event(pane_id, pane_event, ctx)
        });
        ctx.subscribe_to_view(&self.view, move |group, _, event, ctx| {
            group.handle_pane_view_event(pane_id, event, ctx);
        });
    }

    fn detach(
        &self,
        _group: &PaneGroup,
        _detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        let browser_view = self.browser_view(ctx);
        ctx.unsubscribe_to_view(&browser_view);
        ctx.unsubscribe_to_view(&self.view);
    }

    fn snapshot(&self, app: &AppContext) -> LeafContents {
        LeafContents::Browser(self.browser_view(app).as_ref(app).snapshot())
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.browser_view(ctx)
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
