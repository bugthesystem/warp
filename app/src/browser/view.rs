use pathfinder_geometry::rect::RectF;
use warp_browser::{WebView, WebViewEvent};
use warp_errors::report_error;
use warpui::elements::{Empty, SavePosition};
use warpui::text_layout::ClipConfig;
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    WindowId,
};

use super::BrowserViewRegistry;
use super::geometry::webview_bounds;
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::view::{self, HeaderContent, StandardHeader, StandardHeaderOptions};
use crate::pane_group::{BackingView, PaneConfiguration, PaneEvent};

/// Pane title shown until the page reports its own.
const DEFAULT_TITLE: &str = "Browser";

const DEFAULT_URL: &str = "https://www.google.com";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewEvent {
    Pane(PaneEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewAction {}

/// The content of a browser pane. Renders an empty content area and keeps a native web view
/// positioned over it.
pub struct BrowserView {
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
    content_position_id: String,
    url: String,
    title: Option<String>,
    events_tx: async_channel::Sender<WebViewEvent>,
    webview: Option<PlacedWebView>,
}

/// A web view and where it currently sits.
struct PlacedWebView {
    webview: WebView,
    window_id: WindowId,
    /// `None` while the web view is hidden.
    bounds: Option<RectF>,
}

impl BrowserView {
    pub fn new(url: Option<String>, ctx: &mut ViewContext<Self>) -> Self {
        let pane_configuration = ctx.add_model(|_ctx| PaneConfiguration::new(DEFAULT_TITLE));

        let (events_tx, events_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(events_rx, Self::handle_webview_event, |_, _| {});

        let handle = ctx.handle();
        BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.register(handle));

        Self {
            pane_configuration,
            focus_handle: None,
            content_position_id: format!("browser_view_content_{}", ctx.view_id()),
            url: url.unwrap_or_else(|| DEFAULT_URL.to_owned()),
            title: None,
            events_tx,
            webview: None,
        }
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    pub fn focus(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn handle_webview_event(&mut self, event: WebViewEvent, ctx: &mut ViewContext<Self>) {
        match event {
            WebViewEvent::TitleChanged(title) => {
                let title = (!title.is_empty()).then_some(title);
                let pane_title = title.clone().unwrap_or_else(|| DEFAULT_TITLE.to_owned());
                self.pane_configuration.update(ctx, |configuration, ctx| {
                    configuration.set_title(pane_title, ctx)
                });
                self.title = title;
            }
            WebViewEvent::LoadStarted { url } | WebViewEvent::LoadFinished { url } => {
                self.url = url;
            }
        }
        ctx.notify();
    }

    /// Places the web view over the content area as drawn in the last frame. The web view is
    /// created the first time the content area is drawn, recreated if the pane moved to another
    /// window, and hidden while the content area is not drawn.
    pub(super) fn sync_webview(&mut self, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        let target = ctx
            .element_position_by_id_at_last_frame(window_id, &self.content_position_id)
            .map(|rect| webview_bounds(rect, ctx.zoom_factor()));

        if self
            .webview
            .as_ref()
            .is_some_and(|placed| placed.window_id != window_id)
        {
            self.webview = None;
        }

        match (&mut self.webview, target) {
            (None, None) => {}
            (None, Some(bounds)) => self.webview = self.create_webview(window_id, bounds, ctx),
            (Some(placed), target) => placed.move_to(target),
        }
    }

    fn create_webview(
        &self,
        window_id: WindowId,
        bounds: RectF,
        ctx: &AppContext,
    ) -> Option<PlacedWebView> {
        let parent = warp_browser::window_parent(window_id, ctx)?;
        let events_tx = self.events_tx.clone();
        match WebView::new(&parent, &self.url, bounds, move |event| {
            // Fails only once the view, and with it the receiver, is gone.
            let _ = events_tx.try_send(event);
        }) {
            Ok(webview) => Some(PlacedWebView {
                webview,
                window_id,
                bounds: Some(bounds),
            }),
            Err(err) => {
                report_error!(anyhow::Error::new(err).context("Failed to create browser web view"));
                None
            }
        }
    }
}

impl PlacedWebView {
    fn move_to(&mut self, target: Option<RectF>) {
        if self.bounds == target {
            return;
        }
        let result = match target {
            Some(bounds) => self.webview.set_bounds(bounds).and_then(|()| {
                if self.bounds.is_none() {
                    self.webview.set_visible(true)
                } else {
                    Ok(())
                }
            }),
            None => self.webview.set_visible(false),
        };
        if let Err(err) = result {
            report_error!(
                anyhow::Error::new(err).context("Failed to place browser web view"),
                ReportErrorLogMode::OncePerRun
            );
        }
        self.bounds = target;
    }
}

impl Entity for BrowserView {
    type Event = BrowserViewEvent;
}

impl View for BrowserView {
    fn ui_name() -> &'static str {
        "BrowserView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        // Saved for a single frame so the position is absent whenever the pane is not drawn,
        // which is what hides the web view.
        SavePosition::new(Empty::new().finish(), &self.content_position_id)
            .for_single_frame()
            .finish()
    }
}

impl TypedActionView for BrowserView {
    type Action = BrowserViewAction;

    fn handle_action(&mut self, action: &Self::Action, _ctx: &mut ViewContext<Self>) {
        match *action {}
    }
}

impl BackingView for BrowserView {
    type PaneHeaderOverflowMenuAction = BrowserViewAction;
    type CustomAction = BrowserViewAction;
    type AssociatedData = ();

    fn handle_pane_header_overflow_menu_action(
        &mut self,
        action: &Self::PaneHeaderOverflowMenuAction,
        _ctx: &mut ViewContext<Self>,
    ) {
        match *action {}
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(BrowserViewEvent::Pane(PaneEvent::Close));
    }

    fn focus_contents(&mut self, ctx: &mut ViewContext<Self>) {
        self.focus(ctx);
    }

    fn render_header_content(
        &self,
        _ctx: &view::HeaderRenderContext<'_>,
        _app: &AppContext,
    ) -> HeaderContent {
        HeaderContent::Standard(StandardHeader {
            title: self
                .title
                .clone()
                .unwrap_or_else(|| DEFAULT_TITLE.to_owned()),
            title_secondary: Some(self.url.clone()),
            title_style: None,
            title_clip_config: ClipConfig::end(),
            title_max_width: None,
            left_of_title: None,
            right_of_title: None,
            left_of_overflow: None,
            options: StandardHeaderOptions::default(),
        })
    }

    fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, _ctx: &mut ViewContext<Self>) {
        self.focus_handle = Some(focus_handle);
    }
}
