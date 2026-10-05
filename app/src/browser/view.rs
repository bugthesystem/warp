use std::time::Duration;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warp_browser::annotation::{PageAnnotation, annotate_script, format_annotations};
use warp_browser::local_servers::LocalServer;
use warp_browser::{WebView, WebViewEvent};
use warp_core::ui::appearance::Appearance;
use warp_errors::report_error;
use warpui::r#async::Timer;
use warpui::clipboard::{ClipboardContent, ImageData};
use warpui::elements::{
    Align, Border, ChildAnchor, Clipped, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Empty, Expanded, Flex, Hoverable, MainAxisSize, MouseStateHandle,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, SavePosition,
    Shrinkable, Stack, Text,
};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    AppContext, BlurContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View,
    ViewContext, ViewHandle, WindowId,
};

use super::geometry::webview_bounds;
use super::{BrowserHistoryModel, BrowserViewRegistry};
use crate::app_state::BrowserPaneSnapshot;
use crate::editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions, TextOptions};
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::PaneHeaderAction;
use crate::pane_group::pane::view::header::components::render_pane_header_buttons;
use crate::pane_group::pane::view::header::render_pane_header_draggable;
use crate::pane_group::pane::view::{self, HeaderContent};
use crate::pane_group::{BackingView, PaneConfiguration, PaneEvent};
use crate::ui_components::buttons::icon_button;
use crate::ui_components::icons::Icon;

/// Title shown for a tab until its page reports one.
const DEFAULT_TITLE: &str = "Browser";

const NEW_TAB_TITLE: &str = "New Tab";

/// Most recent sites listed on the new-tab page.
const MAX_RECENTS: usize = 8;
const MAX_LOCAL_SERVERS: usize = 6;
const NOTICE_DURATION: Duration = Duration::from_secs(2);
/// How long the agent activity bar stays after an agent's last step.
const AGENT_ACTIVITY_LINGER: Duration = Duration::from_secs(4);
const MAX_AGENT_STEPS: usize = 5;

const URL_FIELD_PLACEHOLDER: &str = "Search or enter URL";

const TOOLBAR_PADDING: f32 = 4.;

const TAB_MAX_WIDTH: f32 = 220.;

const TAB_ICON_SIZE: f32 = 14.;

const STATUS_ICON_SIZE: f32 = 16.;

const CORNER_RADIUS: f32 = 6.;

const NEW_TAB_PAGE_WIDTH: f32 = 560.;

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
    CloseActiveTab,
    FocusUrlField,
    OpenUrl(String),
    ResolveAgentApproval(AgentApproval),
    ToggleAgentAutoApprove,
    ToggleAnnotate,
    CopyScreenshot,
    SetAgentPaused(bool),
    StopAgent,
}

/// The user's answer when an agent asks to use a site that is not local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentApproval {
    Once,
    Always,
    Deny,
}

/// Header clicks reach the view as custom pane header actions, since the header is not one of the
/// view's descendants.
type HeaderAction = PaneHeaderAction<BrowserHeaderAction, BrowserViewAction>;

/// Actions for the pane header's overflow menu, which has no items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserHeaderAction {}

/// The content of a browser pane: a toolbar, and below it an empty content area with the active
/// tab's native web view positioned over it. The pane header shows the tab strip.
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
    /// Whether the active page is in annotate mode, where clicking an element adds a note for
    /// agents instead of acting on it.
    annotating: bool,
    annotate_button: MouseStateHandle,
    screenshot_button: MouseStateHandle,
    /// Confirmation shown briefly in the toolbar.
    notice: Option<&'static str>,
    /// What agents did in this pane recently, oldest first. Cleared once they go quiet.
    agent_steps: Vec<String>,
    /// Bumped with each step, so only the latest step's timer clears the steps.
    agent_step_generation: u64,
    pause_agent_button: MouseStateHandle,
    stop_agent_button: MouseStateHandle,
    /// A site an agent is waiting for the user to approve.
    agent_approval_site: Option<String>,
    approve_once_button: MouseStateHandle,
    approve_always_button: MouseStateHandle,
    deny_button: MouseStateHandle,
    auto_approve_button: MouseStateHandle,
    /// One per row the new-tab page can show, across its sections.
    recent_buttons: Vec<MouseStateHandle>,
    /// Servers found listening on this machine when a new-tab page was last shown.
    local_servers: Vec<LocalServer>,
    local_server_buttons: Vec<MouseStateHandle>,
    events_tx: async_channel::Sender<(u64, WebViewEvent)>,
}

/// One page in a browser pane. Agents address tabs by `id`.
struct BrowserTab {
    id: u64,
    /// Empty while the tab shows the new-tab page.
    url: String,
    /// Whether an agent opened or navigated the tab, which history records with its visits.
    agent_driven: bool,
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
    /// Creates a view with one tab showing `url`, or the new-tab page.
    pub fn new(url: Option<String>, ctx: &mut ViewContext<Self>) -> Self {
        Self::with_tabs(&[url.unwrap_or_default()], 0, ctx)
    }

    /// Creates a view with a tab per URL, where an empty URL is a new-tab page, and the given tab
    /// active.
    pub fn with_tabs(
        tab_urls: &[String],
        active_tab_index: usize,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let pane_configuration = ctx.add_model(|_ctx| PaneConfiguration::new(DEFAULT_TITLE));

        let appearance = Appearance::as_ref(ctx);
        let url_text = TextOptions::ui_text(Some(appearance.ui_font_size()), appearance);
        let url_editor = ctx.add_typed_action_view(|ctx| {
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: url_text,
                    ..Default::default()
                },
                ctx,
            );
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
            annotating: false,
            annotate_button: MouseStateHandle::default(),
            screenshot_button: MouseStateHandle::default(),
            notice: None,
            agent_steps: Vec::new(),
            agent_step_generation: 0,
            pause_agent_button: MouseStateHandle::default(),
            stop_agent_button: MouseStateHandle::default(),
            agent_approval_site: None,
            approve_once_button: MouseStateHandle::default(),
            approve_always_button: MouseStateHandle::default(),
            deny_button: MouseStateHandle::default(),
            auto_approve_button: MouseStateHandle::default(),
            recent_buttons: (0..MAX_RECENTS * 3)
                .map(|_| MouseStateHandle::default())
                .collect(),
            local_servers: Vec::new(),
            local_server_buttons: (0..MAX_LOCAL_SERVERS)
                .map(|_| MouseStateHandle::default())
                .collect(),
            events_tx,
        };
        #[cfg(not(target_family = "wasm"))]
        ctx.observe(&super::BrowserAgent::handle(ctx), |_, _, ctx| ctx.notify());
        for url in tab_urls {
            view.open_tab((!url.is_empty()).then(|| url.clone()), false, ctx);
        }
        if view.tabs.is_empty() {
            view.open_tab(None, false, ctx);
        }
        view.activate(active_tab_index.min(view.tabs.len() - 1), ctx);
        view
    }

    /// The pane's tabs, for saving across restarts.
    pub fn snapshot(&self) -> BrowserPaneSnapshot {
        BrowserPaneSnapshot {
            tab_urls: self.tabs.iter().map(|tab| tab.url.clone()).collect(),
            active_tab_index: self.active_tab,
        }
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    /// Focuses the page when it has been shown, and the URL field otherwise.
    pub fn focus(&mut self, ctx: &mut ViewContext<Self>) {
        if self.active().url.is_empty() {
            ctx.focus(&self.url_editor);
            #[cfg(not(target_family = "wasm"))]
            self.refresh_local_servers(ctx);
            return;
        }
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

    /// Asks the user, in this pane, whether an agent may use `site`.
    pub fn request_agent_approval(&mut self, site: String, ctx: &mut ViewContext<Self>) {
        self.agent_approval_site = Some(site);
        ctx.notify();
    }

    /// Shows that an agent is taking `step` in this pane.
    pub fn record_agent_step(&mut self, step: String, ctx: &mut ViewContext<Self>) {
        self.agent_steps.push(step);
        if self.agent_steps.len() > MAX_AGENT_STEPS {
            self.agent_steps.remove(0);
        }
        self.agent_step_generation += 1;
        let generation = self.agent_step_generation;
        ctx.spawn(Timer::after(AGENT_ACTIVITY_LINGER), move |me, _, ctx| {
            if me.agent_step_generation == generation {
                me.agent_steps.clear();
                ctx.notify();
            }
        });
        ctx.notify();
    }

    pub fn clear_agent_approval(&mut self, ctx: &mut ViewContext<Self>) {
        self.agent_approval_site = None;
        ctx.notify();
    }

    /// Opens a tab showing `url`, or the new-tab page, and makes it active. Returns its id.
    pub fn open_tab(
        &mut self,
        url: Option<String>,
        agent_driven: bool,
        ctx: &mut ViewContext<Self>,
    ) -> u64 {
        let id = BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.new_tab_id());
        self.tabs.push(BrowserTab {
            id,
            url: url.unwrap_or_default(),
            agent_driven,
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

    /// Records that an agent drives the tab, so its visits are listed as opened by agents.
    pub fn mark_agent_driven(&mut self, tab_id: u64) {
        if let Some(index) = self.tab_index(tab_id) {
            self.tabs[index].agent_driven = true;
        }
    }

    /// Loads `url` in the given tab, at the request of an agent when `by_agent` is set.
    pub fn load_url(
        &mut self,
        tab_id: u64,
        url: String,
        by_agent: bool,
        ctx: &mut ViewContext<Self>,
    ) {
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
        tab.agent_driven = by_agent;
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
        self.set_annotating(false, ctx);
        self.unfocus_page();
        self.active_tab = index;
        let tab_id = self.active().id;
        BrowserViewRegistry::handle(ctx)
            .update(ctx, |registry, _| registry.set_current_tab(tab_id));
        self.show_url(ctx);
        self.sync_chrome(ctx);
        #[cfg(not(target_family = "wasm"))]
        if self.active().url.is_empty() {
            self.refresh_local_servers(ctx);
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn refresh_local_servers(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            super::local_servers::detect_local_servers(),
            |me, servers, ctx| {
                me.local_servers = servers;
                ctx.notify();
            },
        );
    }

    /// Turns annotate mode on or off in the active page. Turning it on gives the page keyboard
    /// focus so it receives Escape.
    fn set_annotating(&mut self, annotating: bool, ctx: &mut ViewContext<Self>) {
        if self.annotating == annotating {
            return;
        }
        let Some(placed) = &self.active().webview else {
            return;
        };
        if let Err(err) = placed
            .webview
            .evaluate(&annotate_script(annotating), |_| {})
        {
            report_error!(anyhow::Error::new(err).context("Failed to toggle annotate mode"));
            return;
        }
        self.annotating = annotating;
        if annotating {
            self.focus(ctx);
        }
        ctx.notify();
    }

    /// Shows `message` in the toolbar for a moment. A toast would be drawn over the page, which
    /// hides it.
    fn show_notice(&mut self, message: &'static str, ctx: &mut ViewContext<Self>) {
        self.notice = Some(message);
        ctx.notify();
        ctx.spawn(Timer::after(NOTICE_DURATION), move |me, _, ctx| {
            if me.notice == Some(message) {
                me.notice = None;
                ctx.notify();
            }
        });
    }

    /// Copies a screenshot of the active page to the clipboard, for pasting into an agent.
    fn copy_screenshot(&self, ctx: &mut ViewContext<Self>) {
        let Some(placed) = &self.active().webview else {
            return;
        };
        let (png_tx, png_rx) = futures::channel::oneshot::channel();
        placed.webview.snapshot_png(move |png| {
            let _ = png_tx.send(png);
        });
        ctx.spawn(
            async move { png_rx.await.ok().flatten() },
            |me, png, ctx| {
                let message = match png {
                    Some(data) => {
                        ctx.clipboard().write(ClipboardContent {
                            images: Some(vec![ImageData {
                                data,
                                mime_type: "image/png".to_owned(),
                                filename: Some("screenshot.png".to_owned()),
                            }]),
                            ..Default::default()
                        });
                        "Screenshot copied"
                    }
                    None => "Couldn't take a screenshot",
                };
                me.show_notice(message, ctx);
            },
        );
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
        let title = tab_title(active);
        self.pane_configuration.update(ctx, |configuration, ctx| {
            configuration.set_title(title, ctx)
        });
        if !self.url_editor.is_focused(ctx) {
            self.show_url(ctx);
        }
        ctx.notify();
    }

    /// Shows the active tab's URL in the URL field: in full while the field has focus, and
    /// shortened otherwise.
    fn show_url(&mut self, ctx: &mut ViewContext<Self>) {
        let url = &self.active().url;
        let text = if self.url_editor.is_focused(ctx) {
            url.clone()
        } else {
            warp_browser::display_url(url)
        };
        self.url_editor
            .update(ctx, |editor, ctx| editor.set_buffer_text(&text, ctx));
    }

    fn handle_url_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Enter => {
                let input = self.url_editor.as_ref(ctx).buffer_text(ctx);
                let tab_id = self.active().id;
                self.load_url(tab_id, warp_browser::resolve_input(&input), false, ctx);
            }
            EditorEvent::Escape => self.show_url(ctx),
            EditorEvent::Focused => {
                self.unfocus_page();
                self.show_url(ctx);
                self.url_editor
                    .update(ctx, |editor, ctx| editor.select_all(ctx));
            }
            EditorEvent::Blurred => self.show_url(ctx),
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
                let url = tab.url.clone();
                BrowserHistoryModel::handle(ctx)
                    .update(ctx, |history, ctx| history.set_title(&url, &title, ctx));
                tab.title = (!title.is_empty()).then_some(title);
            }
            WebViewEvent::LoadStarted { url } => {
                tab.is_loading = true;
                tab.url = url;
                if index == self.active_tab {
                    self.annotating = false;
                }
            }
            WebViewEvent::LoadFinished { url } => {
                tab.is_loading = false;
                let opened_by_agent = tab.agent_driven;
                BrowserHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                    history.record_visit(&url, opened_by_agent, ctx)
                });
                tab.url = url;
                if let Some(placed) = &tab.webview {
                    tab.can_go_back = placed.webview.can_go_back().unwrap_or(false);
                    tab.can_go_forward = placed.webview.can_go_forward().unwrap_or(false);
                }
            }
            WebViewEvent::PageFocused => {}
            WebViewEvent::Annotation(json) => {
                if let Some(annotation) = PageAnnotation::parse(&json) {
                    ctx.clipboard()
                        .write(ClipboardContent::plain_text(format_annotations(
                            std::slice::from_ref(&annotation),
                        )));
                    #[cfg(not(target_family = "wasm"))]
                    super::BrowserAgent::handle(ctx)
                        .update(ctx, |agent, ctx| agent.add_annotation(annotation, ctx));
                    self.show_notice("Note copied for your agent", ctx);
                }
            }
            WebViewEvent::AnnotateExited => {
                if index == self.active_tab {
                    self.annotating = false;
                }
            }
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

    /// The tab strip shown in the pane header, with the pane's own header buttons on the right.
    fn render_header_tabs(
        &self,
        header_ctx: &view::HeaderRenderContext<'_>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let is_pane_dragging = header_ctx.draggable_state.is_dragging();

        let mut tabs_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(TOOLBAR_PADDING)
            .with_main_axis_size(if is_pane_dragging {
                MainAxisSize::Min
            } else {
                MainAxisSize::Max
            });
        for (index, tab) in self.tabs.iter().enumerate() {
            let chip = self.render_tab_chip(tab, index == self.active_tab, appearance);
            // Dragging lays the header out without width limits, which shrinkable children
            // cannot handle.
            tabs_row.add_child(if is_pane_dragging {
                ConstrainedBox::new(chip)
                    .with_max_width(TAB_MAX_WIDTH)
                    .finish()
            } else {
                Shrinkable::new(
                    1.,
                    ConstrainedBox::new(chip)
                        .with_max_width(TAB_MAX_WIDTH)
                        .finish(),
                )
                .finish()
            });
        }
        tabs_row.add_child(header_button(
            appearance,
            Icon::Plus,
            &self.new_tab_button,
            BrowserViewAction::NewTab,
        ));

        let draggable_spacer = render_pane_header_draggable::<BrowserView>(
            self.pane_configuration.clone(),
            Empty::new().finish(),
            header_ctx.draggable_state.clone(),
            app,
        );
        tabs_row.add_child(if is_pane_dragging {
            draggable_spacer
        } else {
            Expanded::new(1., draggable_spacer).finish()
        });

        let clipped_tabs = Clipped::new(tabs_row.finish()).finish();
        let show_close_button = self
            .focus_handle
            .as_ref()
            .is_some_and(|handle| handle.is_in_split_pane(app));
        let buttons = render_pane_header_buttons::<BrowserHeaderAction, BrowserViewAction>(
            header_ctx,
            appearance,
            show_close_button,
            None,
            None,
        );

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(if is_pane_dragging {
                    clipped_tabs
                } else {
                    Expanded::new(1., clipped_tabs).finish()
                })
                .with_child(Align::new(buttons).finish())
                .finish(),
        )
        .with_horizontal_padding(TOOLBAR_PADDING)
        .finish()
    }

    fn render_tab_chip(
        &self,
        tab: &BrowserTab,
        is_active: bool,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let tab_id = tab.id;
        let title = tab_title(tab);
        let text_color = if is_active {
            theme.active_ui_text_color()
        } else {
            theme.nonactive_ui_text_color()
        };
        let icon = if tab.is_loading {
            Icon::Loading
        } else {
            Icon::Globe
        };
        let icon = ConstrainedBox::new(icon.to_warpui_icon(text_color).finish())
            .with_width(TAB_ICON_SIZE)
            .with_height(TAB_ICON_SIZE)
            .finish();
        let label = Text::new_inline(
            title,
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(text_color.into())
        .finish();
        let close_button = tab.close_button.clone();
        let close = header_button(
            appearance,
            Icon::X,
            &tab.close_button,
            BrowserViewAction::CloseTab(tab_id),
        );
        let background = is_active.then(|| theme.surface_2());

        Hoverable::new(tab.tab_button.clone(), move |state| {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(6.)
                .with_child(icon)
                .with_child(Shrinkable::new(1., label).finish());
            if is_active || state.is_hovered() {
                row.add_child(close);
            }
            let mut container = Container::new(row.finish())
                .with_horizontal_padding(8.)
                .with_vertical_padding(2.)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CORNER_RADIUS)));
            if let Some(background) = background {
                container = container.with_background(background);
            }
            container.finish()
        })
        .on_click(move |ctx, _, _| {
            let close_hovered = close_button.lock().is_ok_and(|handle| handle.is_hovered());
            if !close_hovered {
                ctx.dispatch_typed_action(HeaderAction::CustomAction(
                    BrowserViewAction::SelectTab(tab_id),
                ));
            }
        })
        .on_middle_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(HeaderAction::CustomAction(BrowserViewAction::CloseTab(
                tab_id,
            )));
        })
        .finish()
    }

    /// Shown in place of a page while a tab has no URL: servers running locally, local apps, pages
    /// agents opened, and other recent pages.
    fn render_new_tab_page(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let sections = BrowserHistoryModel::as_ref(app).sections(MAX_RECENTS);
        let mut mouse_states = self.recent_buttons.iter();

        let mut column = Flex::column().with_spacing(2.);
        let groups = [
            (Icon::Laptop, "Local apps", &sections.local_apps),
            (
                Icon::AgentMode,
                "Opened by agents",
                &sections.opened_by_agents,
            ),
            (Icon::Clock, "Recent", &sections.recent),
        ];
        let mut is_empty = self.local_servers.is_empty();
        if !is_empty {
            column.add_child(
                Container::new(section_heading(
                    Icon::Terminal,
                    "Running on this machine",
                    appearance,
                ))
                .with_margin_top(14.)
                .with_margin_bottom(4.)
                .finish(),
            );
            for (server, mouse_state) in self.local_servers.iter().zip(&self.local_server_buttons) {
                column.add_child(history_row(
                    &server.url(),
                    Some(&server.process),
                    Icon::Terminal,
                    mouse_state,
                    appearance,
                ));
            }
        }
        for (icon, heading, entries) in groups {
            if entries.is_empty() {
                continue;
            }
            is_empty = false;
            column.add_child(
                Container::new(section_heading(icon, heading, appearance))
                    .with_margin_top(14.)
                    .with_margin_bottom(4.)
                    .finish(),
            );
            for (entry, mouse_state) in entries.iter().zip(mouse_states.by_ref()) {
                column.add_child(history_row(
                    &entry.url,
                    entry.title.as_deref(),
                    icon,
                    mouse_state,
                    appearance,
                ));
            }
        }
        if is_empty {
            column.add_child(
                Text::new_inline(
                    "Pages you and your agents visit appear here. Type a URL or search above.",
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(theme.nonactive_ui_text_color().into())
                .finish(),
            );
        }

        Align::new(
            Container::new(
                ConstrainedBox::new(column.finish())
                    .with_max_width(NEW_TAB_PAGE_WIDTH)
                    .finish(),
            )
            .with_margin_top(32.)
            .with_horizontal_padding(16.)
            .finish(),
        )
        .top_center()
        .finish()
    }

    /// What the agent is doing, with controls to pause it, hand control back, or stop it.
    fn render_agent_activity(&self, paused: bool, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let status = if paused {
            "Agent paused. You're in control.".to_owned()
        } else {
            match self.agent_steps.last() {
                Some(step) => format!("Agent: {step}"),
                None => "Agent".to_owned(),
            }
        };
        let (toggle_label, toggle_action) = if paused {
            ("Resume", BrowserViewAction::SetAgentPaused(false))
        } else {
            ("Pause", BrowserViewAction::SetAgentPaused(true))
        };
        let button = |label: &str, mouse_state: &MouseStateHandle, action: BrowserViewAction| {
            appearance
                .ui_builder()
                .button(ButtonVariant::Secondary, mouse_state.clone())
                .with_centered_text_label(label.to_owned())
                .build()
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
                .finish()
        };
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(8.)
                .with_child(
                    ConstrainedBox::new(Icon::AgentMode.to_warpui_icon(theme.accent()).finish())
                        .with_width(TAB_ICON_SIZE)
                        .with_height(TAB_ICON_SIZE)
                        .finish(),
                )
                .with_child(
                    Shrinkable::new(
                        1.,
                        Text::new_inline(
                            status,
                            appearance.ui_font_family(),
                            appearance.ui_font_size(),
                        )
                        .with_color(theme.active_ui_text_color().into())
                        .finish(),
                    )
                    .finish(),
                )
                .with_child(button(
                    toggle_label,
                    &self.pause_agent_button,
                    toggle_action,
                ))
                .with_child(button(
                    "Stop",
                    &self.stop_agent_button,
                    BrowserViewAction::StopAgent,
                ))
                .finish(),
        )
        .with_horizontal_padding(12.)
        .with_vertical_padding(6.)
        .with_background(theme.accent().with_opacity(12))
        .with_border(Border::bottom(1.).with_border_fill(theme.accent()))
        .finish()
    }

    /// The prompt asking whether an agent may use a site that is not local.
    fn render_agent_approval(&self, site: &str, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let button = |variant, label: &str, mouse_state: &MouseStateHandle, decision| {
            appearance
                .ui_builder()
                .button(variant, mouse_state.clone())
                .with_centered_text_label(label.to_owned())
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(BrowserViewAction::ResolveAgentApproval(decision))
                })
                .finish()
        };
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(8.)
                .with_child(
                    Shrinkable::new(
                        1.,
                        Text::new_inline(
                            format!("Allow the agent to use {site}?"),
                            appearance.ui_font_family(),
                            appearance.ui_font_size(),
                        )
                        .with_color(theme.active_ui_text_color().into())
                        .finish(),
                    )
                    .finish(),
                )
                .with_child(button(
                    ButtonVariant::Secondary,
                    "Deny",
                    &self.deny_button,
                    AgentApproval::Deny,
                ))
                .with_child(button(
                    ButtonVariant::Secondary,
                    "Allow once",
                    &self.approve_once_button,
                    AgentApproval::Once,
                ))
                .with_child(button(
                    ButtonVariant::Accent,
                    "Always allow",
                    &self.approve_always_button,
                    AgentApproval::Always,
                ))
                .finish(),
        )
        .with_horizontal_padding(12.)
        .with_vertical_padding(8.)
        .with_background(theme.surface_2())
        .with_border(Border::bottom(1.).with_border_fill(theme.outline()))
        .finish()
    }

    fn render_toolbar(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let active = self.active();
        let url_field = appearance
            .ui_builder()
            .text_input(self.url_editor.clone())
            .with_style(UiComponentStyles {
                padding: Some(Coords {
                    top: 5.,
                    bottom: 5.,
                    left: 12.,
                    right: 12.,
                }),
                background: Some(appearance.theme().surface_2().into()),
                border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
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
            .with_children(self.notice.map(|notice| {
                Text::new_inline(
                    notice,
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(appearance.theme().accent().into())
                .finish()
            }))
            .with_child(
                icon_button(
                    appearance,
                    Icon::MessagePlusSquare,
                    self.annotating,
                    self.annotate_button.clone(),
                )
                .build()
                .on_click(|ctx, _, _| ctx.dispatch_typed_action(BrowserViewAction::ToggleAnnotate))
                .finish(),
            )
            .with_child(nav_button(
                appearance,
                Icon::Image,
                &self.screenshot_button,
                active.webview.is_some(),
                BrowserViewAction::CopyScreenshot,
            ))
            .with_child(self.render_auto_approve_switch(appearance, app))
            .finish()
    }

    /// Shows whether agents ask before using sites that are not local, and switches between
    /// asking and approving automatically.
    fn render_auto_approve_switch(
        &self,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        #[cfg(not(target_family = "wasm"))]
        let auto_approve = super::BrowserAgent::as_ref(app).auto_approve();
        #[cfg(target_family = "wasm")]
        let auto_approve = {
            let _ = app;
            false
        };
        let theme = appearance.theme();
        let (label, tooltip) = if auto_approve {
            (
                "Agent: Auto",
                "Agents use any site without asking. Click to ask first.",
            )
        } else {
            (
                "Agent: Ask",
                "Agents ask before using sites that are not local. Click to approve automatically.",
            )
        };
        let (text_color, background, border) = if auto_approve {
            (
                theme.accent(),
                theme.accent().with_opacity(15),
                theme.accent(),
            )
        } else {
            (
                theme.nonactive_ui_text_color(),
                theme.surface_2(),
                theme.outline(),
            )
        };
        let ui_builder = appearance.ui_builder().clone();
        let font_family = appearance.ui_font_family();
        let font_size = appearance.ui_font_size();
        Hoverable::new(self.auto_approve_button.clone(), move |state| {
            let pill = Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(6.)
                    .with_child(
                        ConstrainedBox::new(Icon::AgentMode.to_warpui_icon(text_color).finish())
                            .with_width(TAB_ICON_SIZE)
                            .with_height(TAB_ICON_SIZE)
                            .finish(),
                    )
                    .with_child(
                        Text::new_inline(label, font_family, font_size)
                            .with_color(text_color.into())
                            .finish(),
                    )
                    .finish(),
            )
            .with_horizontal_padding(10.)
            .with_vertical_padding(4.)
            .with_background(background)
            .with_border(Border::all(1.).with_border_fill(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)));
            if state.is_hovered() {
                let mut stack = Stack::new().with_child(pill.finish());
                // Beside the pill rather than below it: anything drawn over the page hides it.
                stack.add_positioned_overlay_child(
                    ui_builder.tool_tip(tooltip.to_owned()).build().finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(-6., 0.),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::MiddleLeft,
                        ChildAnchor::MiddleRight,
                    ),
                );
                stack.finish()
            } else {
                pill.finish()
            }
        })
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(BrowserViewAction::ToggleAgentAutoApprove))
        .finish()
    }
}

fn section_heading(icon: Icon, heading: &str, appearance: &Appearance) -> Box<dyn Element> {
    let color = appearance.theme().nonactive_ui_text_color();
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(6.)
        .with_child(
            ConstrainedBox::new(icon.to_warpui_icon(color).finish())
                .with_width(TAB_ICON_SIZE)
                .with_height(TAB_ICON_SIZE)
                .finish(),
        )
        .with_child(
            Text::new_inline(
                heading.to_owned(),
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_color(color.into())
            .finish(),
        )
        .finish()
}

/// A clickable page on the new-tab page.
fn history_row(
    url: &str,
    title: Option<&str>,
    icon: Icon,
    mouse_state: &MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size();
    let short_url = warp_browser::display_url(url);
    let title = title.map_or_else(|| short_url.clone(), str::to_owned);
    let url = url.to_owned();
    Hoverable::new(mouse_state.clone(), move |state| {
        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(10.)
            .with_child(
                ConstrainedBox::new(
                    icon.to_warpui_icon(theme.nonactive_ui_text_color())
                        .finish(),
                )
                .with_width(TAB_ICON_SIZE)
                .with_height(TAB_ICON_SIZE)
                .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.,
                    Text::new_inline(title, font_family, font_size)
                        .with_color(theme.active_ui_text_color().into())
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.,
                    Text::new_inline(short_url, font_family, font_size)
                        .with_color(theme.nonactive_ui_text_color().into())
                        .finish(),
                )
                .finish(),
            );
        let mut container = Container::new(row.finish())
            .with_horizontal_padding(10.)
            .with_vertical_padding(6.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CORNER_RADIUS)));
        if state.is_hovered() {
            container = container.with_background(theme.surface_2());
        }
        container.finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(BrowserViewAction::OpenUrl(url.clone())))
    .finish()
}

/// Whether a menu, modal or other overlay drew over `rect` in the last frame. The web view sits
/// above everything WarpUI draws, so it has to be hidden for the overlay to show.
fn tab_title(tab: &BrowserTab) -> String {
    match &tab.title {
        Some(title) => title.clone(),
        None if tab.url.is_empty() => NEW_TAB_TITLE.to_owned(),
        None => warp_browser::display_url(&tab.url),
    }
}

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

/// A button in the pane header, which dispatches through the header's custom actions.
fn header_button(
    appearance: &Appearance,
    icon: Icon,
    mouse_state: &MouseStateHandle,
    action: BrowserViewAction,
) -> Box<dyn Element> {
    icon_button(appearance, icon, false, mouse_state.clone())
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(HeaderAction::CustomAction(action.clone()))
        })
        .finish()
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
        #[cfg(not(target_family = "wasm"))]
        let agent_paused = super::BrowserAgent::as_ref(app).is_paused();
        #[cfg(target_family = "wasm")]
        let agent_paused = false;
        let agent_active = agent_paused || !self.agent_steps.is_empty();
        // Saved for a single frame so the position is absent whenever the pane is not drawn,
        // which is what hides the web view.
        let content_area = if self.active().url.is_empty() {
            self.render_new_tab_page(app)
        } else {
            SavePosition::new(Empty::new().finish(), &self.content_position_id)
                .for_single_frame()
                .finish()
        };

        let mut column = Flex::column().with_child(
            Container::new(self.render_toolbar(app))
                .with_uniform_padding(TOOLBAR_PADDING)
                .with_border(Border::bottom(1.).with_border_fill(appearance.theme().outline()))
                .finish(),
        );
        if agent_active {
            column.add_child(self.render_agent_activity(agent_paused, appearance));
        }
        if let Some(site) = &self.agent_approval_site {
            column.add_child(self.render_agent_approval(site, appearance));
        }
        let content_area = if agent_active {
            Container::new(content_area)
                .with_border(Border::all(2.).with_border_fill(appearance.theme().accent()))
                .finish()
        } else {
            content_area
        };
        column
            .with_child(Expanded::new(1., content_area).finish())
            .finish()
    }
}

impl TypedActionView for BrowserView {
    type Action = BrowserViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            BrowserViewAction::NewTab => {
                self.open_tab(None, false, ctx);
                ctx.focus(&self.url_editor);
            }
            BrowserViewAction::SelectTab(tab_id) => self.select_tab(*tab_id, ctx),
            BrowserViewAction::CloseTab(tab_id) => self.close_tab(*tab_id, ctx),
            BrowserViewAction::CloseActiveTab => {
                let tab_id = self.active().id;
                self.close_tab(tab_id, ctx);
            }
            BrowserViewAction::FocusUrlField => ctx.focus(&self.url_editor),
            BrowserViewAction::OpenUrl(url) => {
                let tab_id = self.active().id;
                self.load_url(tab_id, url.clone(), false, ctx);
            }
            BrowserViewAction::ResolveAgentApproval(decision) => {
                let Some(site) = self.agent_approval_site.take() else {
                    return;
                };
                ctx.notify();
                #[cfg(not(target_family = "wasm"))]
                super::BrowserAgent::handle(ctx).update(ctx, |agent, ctx| {
                    agent.resolve_approval(&site, *decision, ctx)
                });
                #[cfg(target_family = "wasm")]
                let _ = (site, decision);
            }
            BrowserViewAction::ToggleAgentAutoApprove => {
                #[cfg(not(target_family = "wasm"))]
                super::BrowserAgent::handle(ctx)
                    .update(ctx, |agent, ctx| agent.toggle_auto_approve(ctx));
            }
            BrowserViewAction::SetAgentPaused(paused) => {
                #[cfg(not(target_family = "wasm"))]
                super::BrowserAgent::handle(ctx)
                    .update(ctx, |agent, ctx| agent.set_paused(*paused, ctx));
                #[cfg(target_family = "wasm")]
                let _ = paused;
            }
            BrowserViewAction::StopAgent => {
                self.agent_steps.clear();
                #[cfg(not(target_family = "wasm"))]
                super::BrowserAgent::handle(ctx).update(ctx, |agent, ctx| agent.stop(ctx));
                ctx.notify();
            }
            BrowserViewAction::ToggleAnnotate => self.set_annotating(!self.annotating, ctx),
            BrowserViewAction::CopyScreenshot => self.copy_screenshot(ctx),
            BrowserViewAction::GoBack => self.navigate_active_tab(WebView::go_back),
            BrowserViewAction::GoForward => self.navigate_active_tab(WebView::go_forward),
            BrowserViewAction::Reload => self.navigate_active_tab(WebView::reload),
        }
    }
}

impl BackingView for BrowserView {
    type PaneHeaderOverflowMenuAction = BrowserHeaderAction;
    type CustomAction = BrowserViewAction;
    type AssociatedData = ();

    fn handle_pane_header_overflow_menu_action(
        &mut self,
        action: &Self::PaneHeaderOverflowMenuAction,
        _ctx: &mut ViewContext<Self>,
    ) {
        match *action {}
    }

    fn handle_custom_action(&mut self, action: &BrowserViewAction, ctx: &mut ViewContext<Self>) {
        self.handle_action(action, ctx);
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(BrowserViewEvent::Pane(PaneEvent::Close));
    }

    fn focus_contents(&mut self, ctx: &mut ViewContext<Self>) {
        self.focus(ctx);
    }

    fn render_header_content(
        &self,
        ctx: &view::HeaderRenderContext<'_>,
        app: &AppContext,
    ) -> HeaderContent {
        HeaderContent::Custom {
            element: self.render_header_tabs(ctx, app),
            has_custom_draggable_behavior: true,
        }
    }

    fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, _ctx: &mut ViewContext<Self>) {
        self.focus_handle = Some(focus_handle);
    }
}
