use pathfinder_geometry::rect::RectF;
use warp_browser::{WebView, WebViewEvent};
use warp_core::ui::appearance::Appearance;
use warp_errors::report_error;
use warpui::elements::{
    Container, CrossAxisAlignment, Empty, Expanded, Flex, MouseStateHandle, ParentElement,
    SavePosition,
};
use warpui::text_layout::ClipConfig;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle, WindowId,
};

use super::BrowserViewRegistry;
use super::geometry::webview_bounds;
use crate::editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions};
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::view::{self, HeaderContent, StandardHeader, StandardHeaderOptions};
use crate::pane_group::{BackingView, PaneConfiguration, PaneEvent};
use crate::ui_components::buttons::icon_button;
use crate::ui_components::icons::Icon;

/// Pane title shown until the page reports its own.
const DEFAULT_TITLE: &str = "Browser";

const DEFAULT_URL: &str = "https://www.google.com";

const URL_FIELD_PLACEHOLDER: &str = "Search or enter URL";

const TOOLBAR_PADDING: f32 = 4.;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewEvent {
    Pane(PaneEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewAction {
    GoBack,
    GoForward,
    Reload,
}

/// Actions for the pane header's overflow menu, which has no items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserHeaderAction {}

/// The content of a browser pane: a toolbar, and below it an empty content area with a native web
/// view positioned over it.
pub struct BrowserView {
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
    content_position_id: String,
    url: String,
    title: Option<String>,
    is_loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    url_editor: ViewHandle<EditorView>,
    back_button: MouseStateHandle,
    forward_button: MouseStateHandle,
    reload_button: MouseStateHandle,
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
        let url = url.unwrap_or_else(|| DEFAULT_URL.to_owned());

        let url_editor = ctx.add_typed_action_view(|ctx| {
            let mut editor = EditorView::single_line(SingleLineEditorOptions::default(), ctx);
            editor.set_placeholder_text(URL_FIELD_PLACEHOLDER, ctx);
            editor.set_buffer_text(&url, ctx);
            editor
        });
        ctx.subscribe_to_view(&url_editor, |me, _, event, ctx| {
            me.handle_url_editor_event(event, ctx);
        });

        let (events_tx, events_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(events_rx, Self::handle_webview_event, |_, _| {});

        let handle = ctx.handle();
        BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.register(handle));

        Self {
            pane_configuration,
            focus_handle: None,
            content_position_id: format!("browser_view_content_{}", ctx.view_id()),
            url,
            title: None,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            url_editor,
            back_button: MouseStateHandle::default(),
            forward_button: MouseStateHandle::default(),
            reload_button: MouseStateHandle::default(),
            events_tx,
            webview: None,
        }
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// The web view, once the pane has been drawn and created it.
    pub fn webview(&self) -> Option<&WebView> {
        self.webview.as_ref().map(|placed| &placed.webview)
    }

    pub fn focus(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.url_editor);
    }

    /// Loads `url` in the web view, or remembers it for when the web view is created.
    pub fn load_url(&mut self, url: String, ctx: &mut ViewContext<Self>) {
        if let Some(placed) = &self.webview
            && let Err(err) = placed.webview.load_url(&url)
        {
            report_error!(anyhow::Error::new(err).context("Failed to load URL in browser pane"));
            return;
        }
        self.set_url(url, ctx);
    }

    fn set_url(&mut self, url: String, ctx: &mut ViewContext<Self>) {
        if !self.url_editor.is_focused(ctx) {
            self.url_editor
                .update(ctx, |editor, ctx| editor.set_buffer_text(&url, ctx));
        }
        self.url = url;
        ctx.notify();
    }

    fn handle_url_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Enter => {
                let input = self.url_editor.as_ref(ctx).buffer_text(ctx);
                self.load_url(warp_browser::resolve_input(&input), ctx);
            }
            EditorEvent::Escape => {
                let url = self.url.clone();
                self.url_editor
                    .update(ctx, |editor, ctx| editor.set_buffer_text(&url, ctx));
            }
            EditorEvent::Focused => {
                self.url_editor
                    .update(ctx, |editor, ctx| editor.select_all(ctx));
            }
            _ => {}
        }
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
            WebViewEvent::LoadStarted { url } => {
                self.is_loading = true;
                self.set_url(url, ctx);
            }
            WebViewEvent::LoadFinished { url } => {
                self.is_loading = false;
                self.refresh_history_state();
                self.set_url(url, ctx);
            }
        }
        ctx.notify();
    }

    fn refresh_history_state(&mut self) {
        let Some(placed) = &self.webview else {
            return;
        };
        self.can_go_back = placed.webview.can_go_back().unwrap_or(false);
        self.can_go_forward = placed.webview.can_go_forward().unwrap_or(false);
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

    fn render_toolbar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let url_field = appearance
            .ui_builder()
            .text_input(self.url_editor.clone())
            .with_style(UiComponentStyles {
                padding: Some(Coords {
                    top: 4.,
                    bottom: 4.,
                    left: 8.,
                    right: 8.,
                }),
                background: Some(appearance.theme().surface_2().into()),
                ..Default::default()
            })
            .build()
            .finish();

        let status_icon = if self.is_loading {
            Icon::Loading
        } else {
            Icon::Globe
        };

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(TOOLBAR_PADDING)
                .with_child(nav_button(
                    appearance,
                    Icon::ArrowLeft,
                    &self.back_button,
                    self.can_go_back,
                    BrowserViewAction::GoBack,
                ))
                .with_child(nav_button(
                    appearance,
                    Icon::ArrowRight,
                    &self.forward_button,
                    self.can_go_forward,
                    BrowserViewAction::GoForward,
                ))
                .with_child(nav_button(
                    appearance,
                    Icon::Refresh,
                    &self.reload_button,
                    true,
                    BrowserViewAction::Reload,
                ))
                .with_child(
                    status_icon
                        .to_warpui_icon(appearance.theme().nonactive_ui_text_color())
                        .finish(),
                )
                .with_child(Expanded::new(1., url_field).finish())
                .finish(),
        )
        .with_uniform_padding(TOOLBAR_PADDING)
        .finish()
    }
}

fn nav_button(
    appearance: &Appearance,
    icon: Icon,
    mouse_state: &MouseStateHandle,
    enabled: bool,
    action: BrowserViewAction,
) -> Box<dyn Element> {
    let mut button = icon_button(appearance, icon, false, mouse_state.clone());
    if !enabled {
        button = button.disabled();
    }
    button
        .build()
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
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

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        // Saved for a single frame so the position is absent whenever the pane is not drawn,
        // which is what hides the web view.
        let content_area = SavePosition::new(Empty::new().finish(), &self.content_position_id)
            .for_single_frame()
            .finish();

        Flex::column()
            .with_child(self.render_toolbar(Appearance::as_ref(app)))
            .with_child(Expanded::new(1., content_area).finish())
            .finish()
    }
}

impl TypedActionView for BrowserView {
    type Action = BrowserViewAction;

    fn handle_action(&mut self, action: &Self::Action, _ctx: &mut ViewContext<Self>) {
        let Some(placed) = &self.webview else {
            return;
        };
        let result = match action {
            BrowserViewAction::GoBack => placed.webview.go_back(),
            BrowserViewAction::GoForward => placed.webview.go_forward(),
            BrowserViewAction::Reload => placed.webview.reload(),
        };
        if let Err(err) = result {
            report_error!(anyhow::Error::new(err).context("Failed to navigate browser pane"));
        }
    }
}

impl BackingView for BrowserView {
    type PaneHeaderOverflowMenuAction = BrowserHeaderAction;
    type CustomAction = BrowserHeaderAction;
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
            title_secondary: None,
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
