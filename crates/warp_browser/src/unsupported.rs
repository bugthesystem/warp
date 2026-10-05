use std::convert::Infallible;

use pathfinder_geometry::rect::RectF;
use warpui::{AppContext, WindowId};

use crate::{Error, WebViewEvent};

/// A native view of a Warp window that web views attach to. Never constructed on this platform.
pub struct WebViewParent(Infallible);

/// Always `None`: web views are not supported on this platform.
pub fn window_parent(_window_id: WindowId, _ctx: &AppContext) -> Option<WebViewParent> {
    None
}

/// A web view. Never constructed on this platform.
pub struct WebView(Infallible);

impl WebView {
    pub fn new(
        parent: &WebViewParent,
        _url: &str,
        _bounds: RectF,
        _on_event: impl Fn(WebViewEvent) + 'static,
    ) -> Result<Self, Error> {
        match parent.0 {}
    }

    pub fn set_bounds(&self, _bounds: RectF) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn set_visible(&self, _visible: bool) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn load_url(&self, _url: &str) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn url(&self) -> Result<String, Error> {
        match self.0 {}
    }

    pub fn go_back(&self) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn go_forward(&self) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn reload(&self) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn focus(&self) -> Result<(), Error> {
        match self.0 {}
    }

    pub fn focus_parent(&self) -> Result<(), Error> {
        match self.0 {}
    }
}
