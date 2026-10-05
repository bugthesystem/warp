use pathfinder_geometry::rect::RectF;
use warp_browser::{WebView, WebViewEvent};
use warp_core::ui::appearance::Appearance;
use warp_errors::report_error;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded, Flex, Hoverable,
    MouseStateHandle, ParentElement, Radius, SavePosition, Text,
};
use warpui::text_layout::ClipConfig;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    AppContext, BlurContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View,
    ViewContext, ViewHandle, WindowId,
};

use super::BrowserViewRegistry;
use super::geometry::webview_bounds;
use crate::editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions};
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::view::{self, HeaderContent, StandardHeader, StandardHeaderOptions};
use crate::pane_group::{BackingView, PaneConfiguration, PaneEvent};
use crate::ui_components::buttons::icon_button;
use crate::ui_components::icons::Icon;

/// Title shown for a tab until its page reports one.
const DEFAULT_TITLE: &str = "Browser";

const DEFAULT_URL: &str = "https://www.google.com";

const URL_FIELD_PLACEHOLDER: &str = "Search or enter URL";

const TOOLBAR_PADDING: f32 = 4.;

const TAB_MAX_WIDTH: f32 = 200.;

const STATUS_ICON_SIZE: f32 = 16.;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewEvent {
    Pane(PaneEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserViewAction {
    GoBack,
    GoForward,
    Reload,
    NewTab,
    SelectTab(u64),
    CloseTab(u64),
}

/// Actions for the pane header's overflow menu, which has no items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserHeaderAction {}

/// The content of a browser pane: a tab strip and a toolbar, and below them an empty content area
/// with the active tab's native web view positioned over it.
pub struct BrowserView {
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
    content_position_id: String,
    tabs: Vec<BrowserTab>,
    active_tab: usize,
    url_editor: ViewHandle<EditorView>,
    back_button: MouseStateHandle,
    forward_button: MouseStateHandle,
    reload_button: MouseStateHandle,
    new_tab_button: MouseStateHandle,
    /// Whether the active page, rather than Warp, has keyboard focus.
    page_focused: bool,
    events_tx: async_channel::Sender<(u64, WebViewEvent)>,
}

/// One page in a browser pane. Agents address tabs by `id`.
struct BrowserTab {
    id: u64,
    url: String,
    title: Option<String>,
    is_loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    webview: Option<PlacedWebView>,
    tab_button: MouseStateHandle,
    close_button: MouseStateHandle,
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

        let url_editor = ctx.add_typed_action_view(|ctx| {
            let mut editor = EditorView::single_line(SingleLineEditorOptions::default(), ctx);
            editor.set_placeholder_text(URL_FIELD_PLACEHOLDER, ctx);
            editor
        });
        ctx.subscribe_to_view(&url_editor, |me, _, event, ctx| {
            me.handle_url_editor_event(event, ctx);
        });

        let (events_tx, events_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(events_rx, Self::handle_webview_event, |_, _| {});

        let handle = ctx.handle();
        BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.register(handle));

        let mut view = Self {
            pane_configuration,
            focus_handle: None,
            content_position_id: format!("browser_view_content_{}", ctx.view_id()),
            tabs: Vec::new(),
            active_tab: 0,
            url_editor,
            back_button: MouseStateHandle::default(),
            forward_button: MouseStateHandle::default(),
            reload_button: MouseStateHandle::default(),
            new_tab_button: MouseStateHandle::default(),
            page_focused: false,
            events_tx,
        };
        view.open_tab(url, ctx);
        view
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    /// Focuses the page when it has been shown, and the URL field otherwise.
    pub fn focus(&mut self, ctx: &mut ViewContext<Self>) {
        match &self.active().webview {
            Some(placed) => {
                ctx.focus_self();
                if let Err(err) = placed.webview.focus() {
                    report_error!(anyhow::Error::new(err).context("Failed to focus browser page"));
                }
                self.page_focused = true;
            }
            None => ctx.focus(&self.url_editor),
        }
    }

    /// Returns keyboard focus from the active page to Warp.
    fn unfocus_page(&mut self) {
        if !std::mem::take(&mut self.page_focused) {
            return;
        }
        if let Some(placed) = &self.active().webview
            && let Err(err) = placed.webview.focus_parent()
        {
            report_error!(anyhow::Error::new(err).context("Failed to unfocus browser page"));
        }
    }

    /// Ids of this pane's tabs, in tab strip order.
    pub fn tab_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.tabs.iter().map(|tab| tab.id)
    }

    pub fn active_tab_id(&self) -> u64 {
        self.active().id
    }

    pub fn tab_title(&self, tab_id: u64) -> Option<&str> {
        self.tab(tab_id).and_then(|tab| tab.title.as_deref())
    }

    pub fn tab_url(&self, tab_id: u64) -> Option<&str> {
        self.tab(tab_id).map(|tab| tab.url.as_str())
    }

    /// The tab's web view, once the tab has been shown and its web view created.
    pub fn tab_webview(&self, tab_id: u64) -> Option<&WebView> {
        self.tab(tab_id)
            .and_then(|tab| tab.webview.as_ref())
            .map(|placed| &placed.webview)
    }

    /// Opens a tab showing `url`, or a default page, and makes it active. Returns its id.
    pub fn open_tab(&mut self, url: Option<String>, ctx: &mut ViewContext<Self>) -> u64 {
        let id = BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.new_tab_id());
        self.tabs.push(BrowserTab {
            id,
            url: url.unwrap_or_else(|| DEFAULT_URL.to_owned()),
            title: None,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            webview: None,
            tab_button: MouseStateHandle::default(),
            close_button: MouseStateHandle::default(),
        });
        self.activate(self.tabs.len() - 1, ctx);
        id
    }

    pub fn select_tab(&mut self, tab_id: u64, ctx: &mut ViewContext<Self>) {
        if let Some(index) = self.tab_index(tab_id) {
            self.activate(index, ctx);
        }
    }

    /// Loads `url` in the given tab.
    pub fn load_url(&mut self, tab_id: u64, url: String, ctx: &mut ViewContext<Self>) {
        let Some(index) = self.tab_index(tab_id) else {
            return;
        };
        let tab = &mut self.tabs[index];
        if let Some(placed) = &tab.webview
            && let Err(err) = placed.webview.load_url(&url)
        {
            report_error!(anyhow::Error::new(err).context("Failed to load URL in browser pane"));
            return;
        }
        tab.url = url;
        self.sync_chrome(ctx);
    }

    fn close_tab(&mut self, tab_id: u64, ctx: &mut ViewContext<Self>) {
        let Some(index) = self.tab_index(tab_id) else {
            return;
        };
        if self.tabs.len() == 1 {
            ctx.emit(BrowserViewEvent::Pane(PaneEvent::Close));
            return;
        }
        self.tabs.remove(index);
        let active = if index < self.active_tab || self.active_tab == self.tabs.len() {
            self.active_tab.saturating_sub(1)
        } else {
            self.active_tab
        };
        self.activate(active, ctx);
    }

    fn activate(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        self.unfocus_page();
        self.active_tab = index;
        let tab_id = self.active().id;
        let url = self.active().url.clone();
        BrowserViewRegistry::handle(ctx)
            .update(ctx, |registry, _| registry.set_current_tab(tab_id));
        self.url_editor
            .update(ctx, |editor, ctx| editor.set_buffer_text(&url, ctx));
        self.sync_chrome(ctx);
    }

    fn navigate_active_tab(&self, navigate: fn(&WebView) -> Result<(), warp_browser::Error>) {
        let Some(placed) = &self.active().webview else {
            return;
        };
        if let Err(err) = navigate(&placed.webview) {
            report_error!(anyhow::Error::new(err).context("Failed to navigate browser pane"));
        }
    }

    fn active(&self) -> &BrowserTab {
        &self.tabs[self.active_tab]
    }

    fn tab(&self, tab_id: u64) -> Option<&BrowserTab> {
        self.tabs.iter().find(|tab| tab.id == tab_id)
    }

    fn tab_index(&self, tab_id: u64) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == tab_id)
    }

    /// Updates the URL field and pane title from the active tab.
    fn sync_chrome(&mut self, ctx: &mut ViewContext<Self>) {
        let active = self.active();
        let title = active
            .title
            .clone()
            .unwrap_or_else(|| DEFAULT_TITLE.to_owned());
        let url = active.url.clone();
        self.pane_configuration.update(ctx, |configuration, ctx| {
            configuration.set_title(title, ctx)
        });
        if !self.url_editor.is_focused(ctx) {
            self.url_editor
                .update(ctx, |editor, ctx| editor.set_buffer_text(&url, ctx));
        }
        ctx.notify();
    }

    fn handle_url_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Enter => {
                let input = self.url_editor.as_ref(ctx).buffer_text(ctx);
                let tab_id = self.active().id;
                self.load_url(tab_id, warp_browser::resolve_input(&input), ctx);
            }
            EditorEvent::Escape => {
                let url = self.active().url.clone();
                self.url_editor
                    .update(ctx, |editor, ctx| editor.set_buffer_text(&url, ctx));
            }
            EditorEvent::Focused => {
                self.unfocus_page();
                self.url_editor
                    .update(ctx, |editor, ctx| editor.select_all(ctx));
            }
            _ => {}
        }
    }

    fn handle_webview_event(
        &mut self,
        (tab_id, event): (u64, WebViewEvent),
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(index) = self.tab_index(tab_id) else {
            return;
        };
        if event == WebViewEvent::PageFocused {
            if index == self.active_tab {
                self.page_focused = true;
                ctx.focus_self();
                ctx.emit(BrowserViewEvent::Pane(PaneEvent::FocusSelf));
            }
            return;
        }
        let tab = &mut self.tabs[index];
        match event {
            WebViewEvent::TitleChanged(title) => {
                tab.title = (!title.is_empty()).then_some(title);
            }
            WebViewEvent::LoadStarted { url } => {
                tab.is_loading = true;
                tab.url = url;
            }
            WebViewEvent::LoadFinished { url } => {
                tab.is_loading = false;
                tab.url = url;
                if let Some(placed) = &tab.webview {
                    tab.can_go_back = placed.webview.can_go_back().unwrap_or(false);
                    tab.can_go_forward = placed.webview.can_go_forward().unwrap_or(false);
                }
            }
            WebViewEvent::PageFocused => {}
        }
        if index == self.active_tab {
            self.sync_chrome(ctx);
        } else {
            ctx.notify();
        }
    }

    /// Places the active tab's web view over the content area as drawn in the last frame, and
    /// hides every other tab's. A web view is created the first time its tab is drawn, recreated
    /// if the pane moved to another window, and hidden while the content area is not drawn or an
    /// overlay covers it.
    pub(super) fn sync_webview(&mut self, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        let target = ctx
            .element_position_by_id_at_last_frame(window_id, &self.content_position_id)
            .filter(|rect| !is_overlaid(window_id, *rect, ctx))
            .map(|rect| webview_bounds(rect, ctx.zoom_factor()));

        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab
                .webview
                .as_ref()
                .is_some_and(|placed| placed.window_id != window_id)
            {
                tab.webview = None;
            }
            let tab_target = target.filter(|_| index == self.active_tab);
            match (&mut tab.webview, tab_target) {
                (None, None) => {}
                (None, Some(bounds)) => {
                    tab.webview = create_webview(tab, window_id, bounds, &self.events_tx, ctx)
                }
                (Some(placed), tab_target) => placed.move_to(tab_target),
            }
        }
    }

    fn render_tab_strip(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let mut strip = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(TOOLBAR_PADDING);

        for (index, tab) in self.tabs.iter().enumerate() {
            let is_active = index == self.active_tab;
            let tab_id = tab.id;
            let title = tab
                .title
                .clone()
                .unwrap_or_else(|| DEFAULT_TITLE.to_owned());
            let label = Text::new_inline(
                title,
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_color(if is_active {
                theme.active_ui_text_color().into()
            } else {
                theme.nonactive_ui_text_color().into()
            })
            .finish();
            let close = icon_button(appearance, Icon::X, false, tab.close_button.clone())
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(BrowserViewAction::CloseTab(tab_id))
                })
                .finish();
            let background = is_active.then(|| theme.surface_2());
            let chip = Hoverable::new(tab.tab_button.clone(), move |_| {
                let mut container = Container::new(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_spacing(TOOLBAR_PADDING)
                        .with_child(Expanded::new(1., label).finish())
                        .with_child(close)
                        .finish(),
                )
                .with_horizontal_padding(8.)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
                if let Some(background) = background {
                    container = container.with_background(background);
                }
                container.finish()
            })
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(BrowserViewAction::SelectTab(tab_id))
            })
            .finish();
            strip = strip.with_child(
                ConstrainedBox::new(chip)
                    .with_max_width(TAB_MAX_WIDTH)
                    .finish(),
            );
        }

        strip
            .with_child(nav_button(
                appearance,
                Icon::Plus,
                &self.new_tab_button,
                true,
                BrowserViewAction::NewTab,
            ))
            .finish()
    }

    fn render_toolbar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let active = self.active();
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

        let status_icon = if active.is_loading {
            Icon::Loading
        } else {
            Icon::Globe
        };

        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(TOOLBAR_PADDING)
            .with_child(nav_button(
                appearance,
                Icon::ArrowLeft,
                &self.back_button,
                active.can_go_back,
                BrowserViewAction::GoBack,
            ))
            .with_child(nav_button(
                appearance,
                Icon::ArrowRight,
                &self.forward_button,
                active.can_go_forward,
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
                ConstrainedBox::new(
                    status_icon
                        .to_warpui_icon(appearance.theme().nonactive_ui_text_color())
                        .finish(),
                )
                .with_width(STATUS_ICON_SIZE)
                .with_height(STATUS_ICON_SIZE)
                .finish(),
            )
            .with_child(Expanded::new(1., url_field).finish())
            .finish()
    }
}

/// Whether a menu, modal or other overlay drew over `rect` in the last frame. The web view sits
/// above everything WarpUI draws, so it has to be hidden for the overlay to show.
fn is_overlaid(window_id: WindowId, rect: RectF, ctx: &AppContext) -> bool {
    ctx.presenter(window_id).is_some_and(|presenter| {
        presenter
            .borrow()
            .scene()
            .is_some_and(|scene| scene.is_overlaid(rect))
    })
}

fn create_webview(
    tab: &BrowserTab,
    window_id: WindowId,
    bounds: RectF,
    events_tx: &async_channel::Sender<(u64, WebViewEvent)>,
    ctx: &AppContext,
) -> Option<PlacedWebView> {
    let parent = warp_browser::window_parent(window_id, ctx)?;
    let events_tx = events_tx.clone();
    let tab_id = tab.id;
    match WebView::new(&parent, &tab.url, bounds, move |event| {
        // Fails only once the view, and with it the receiver, is gone.
        let _ = events_tx.try_send((tab_id, event));
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

    fn on_blur(&mut self, blur_ctx: &BlurContext, _ctx: &mut ViewContext<Self>) {
        // AppKit moves keyboard focus on clicks, but not when Warp moves focus with the keyboard,
        // such as when navigating between panes.
        if blur_ctx.is_self_blurred() {
            self.unfocus_page();
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        // Saved for a single frame so the position is absent whenever the pane is not drawn,
        // which is what hides the web view.
        let content_area = SavePosition::new(Empty::new().finish(), &self.content_position_id)
            .for_single_frame()
            .finish();

        Flex::column()
            .with_child(
                Container::new(
                    Flex::column()
                        .with_spacing(TOOLBAR_PADDING)
                        .with_child(self.render_tab_strip(appearance))
                        .with_child(self.render_toolbar(appearance))
                        .finish(),
                )
                .with_uniform_padding(TOOLBAR_PADDING)
                .finish(),
            )
            .with_child(Expanded::new(1., content_area).finish())
            .finish()
    }
}

impl TypedActionView for BrowserView {
    type Action = BrowserViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            BrowserViewAction::NewTab => {
                self.open_tab(None, ctx);
                ctx.focus(&self.url_editor);
            }
            BrowserViewAction::SelectTab(tab_id) => self.select_tab(*tab_id, ctx),
            BrowserViewAction::CloseTab(tab_id) => self.close_tab(*tab_id, ctx),
            BrowserViewAction::GoBack => self.navigate_active_tab(WebView::go_back),
            BrowserViewAction::GoForward => self.navigate_active_tab(WebView::go_forward),
            BrowserViewAction::Reload => self.navigate_active_tab(WebView::reload),
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
                .active()
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
