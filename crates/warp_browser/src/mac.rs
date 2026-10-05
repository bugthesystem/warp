use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use pathfinder_geometry::rect::RectF;
use warpui::platform::mac::WindowExt;
use warpui::{AppContext, WindowId};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use wry::{PageLoadEvent, Rect, WebViewBuilder};

use crate::{Error, WebViewEvent};

/// A native view of a Warp window that web views attach to.
pub struct WebViewParent {
    view: Retained<NSView>,
}

impl HasWindowHandle for WebViewParent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = AppKitWindowHandle::new(NonNull::from(&*self.view).cast());
        // SAFETY: `self.view` is retained, so the view outlives the returned borrow.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
    }
}

/// Returns the content view of the given window, or `None` if it has no native window.
pub fn window_parent(app: &AppContext, window_id: WindowId) -> Option<WebViewParent> {
    let window = app.windows().platform_window(window_id)?;
    let view = window.as_ref().content_view()?;
    Some(WebViewParent { view })
}

/// A WKWebView attached as a child of a Warp window's content view. Dropping it removes the web
/// view from the window.
pub struct WebView {
    webview: wry::WebView,
}

impl WebView {
    /// Creates a web view at `bounds` (in logical pixels, relative to the parent's top-left
    /// corner) and starts loading `url`. `on_event` is called on the main thread.
    pub fn new(
        parent: &WebViewParent,
        url: &str,
        bounds: RectF,
        on_event: impl Fn(WebViewEvent) + 'static,
    ) -> Result<Self, Error> {
        let on_event: Rc<dyn Fn(WebViewEvent)> = Rc::new(on_event);
        let on_title_changed = on_event.clone();
        let on_page_load = on_event;

        let webview = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(to_wry_rect(bounds))
            .with_devtools(cfg!(debug_assertions))
            .with_back_forward_navigation_gestures(true)
            .with_document_title_changed_handler(move |title| {
                on_title_changed(WebViewEvent::TitleChanged(title));
            })
            .with_on_page_load_handler(move |event, url| {
                on_page_load(match event {
                    PageLoadEvent::Started => WebViewEvent::LoadStarted { url },
                    PageLoadEvent::Finished => WebViewEvent::LoadFinished { url },
                });
            })
            .build_as_child(parent)?;

        Ok(Self { webview })
    }

    /// Moves the web view to `bounds`, in logical pixels relative to the parent's top-left corner.
    pub fn set_bounds(&self, bounds: RectF) -> Result<(), Error> {
        Ok(self.webview.set_bounds(to_wry_rect(bounds))?)
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), Error> {
        Ok(self.webview.set_visible(visible)?)
    }

    pub fn load_url(&self, url: &str) -> Result<(), Error> {
        Ok(self.webview.load_url(url)?)
    }

    pub fn url(&self) -> Result<String, Error> {
        Ok(self.webview.url()?)
    }

    pub fn go_back(&self) -> Result<(), Error> {
        Ok(self.webview.go_back()?)
    }

    pub fn go_forward(&self) -> Result<(), Error> {
        Ok(self.webview.go_forward()?)
    }

    pub fn reload(&self) -> Result<(), Error> {
        Ok(self.webview.reload()?)
    }

    /// Gives the web view keyboard focus.
    pub fn focus(&self) -> Result<(), Error> {
        Ok(self.webview.focus()?)
    }

    /// Returns keyboard focus to the Warp window.
    pub fn focus_parent(&self) -> Result<(), Error> {
        Ok(self.webview.focus_parent()?)
    }
}

fn to_wry_rect(bounds: RectF) -> Rect {
    Rect {
        position: LogicalPosition::new(bounds.origin_x(), bounds.origin_y()).into(),
        size: LogicalSize::new(bounds.width(), bounds.height()).into(),
    }
}
