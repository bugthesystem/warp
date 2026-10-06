use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSEvent, NSEventModifierFlags, NSEventType, NSImage,
    NSView,
};
use objc2_foundation::{NSDictionary, NSError, NSPoint, NSProcessInfo, NSString};
use pathfinder_geometry::rect::RectF;
use warpui::platform::mac::WindowExt;
use warpui::{AppContext, WindowId};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use wry::{PageLoadEvent, Rect, WebViewBuilder, WebViewExtMacOS};

use crate::agent::CONSOLE_CAPTURE_SCRIPT;
use crate::annotation::{ANNOTATE_EXITED_MESSAGE, ANNOTATION_MESSAGE_PREFIX};
use crate::{Error, PAGE_ACTION_PREFIX, WebViewEvent};

/// Message the focus script posts when the user clicks in the page.
const PAGE_FOCUSED_MESSAGE: &str = "warp:page-focused";

/// Reports clicks in the page so Warp can move its own focus to the pane. The capture phase sees
/// the click even when the page stops it from propagating. Clicks an agent makes are skipped, so
/// they do not pull focus from the user's terminal.
const FOCUS_SCRIPT: &str = r#"document.addEventListener("mousedown", () => { if ((window.__warpAgentInputUntil || 0) > Date.now()) return; window.ipc.postMessage("warp:page-focused"); }, true);"#;

/// The virtual key code of Return.
const RETURN_KEY_CODE: u16 = 36;

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
pub fn window_parent(window_id: WindowId, ctx: &AppContext) -> Option<WebViewParent> {
    let window = ctx.windows().platform_window(window_id)?;
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
        let on_ipc = on_event.clone();
        let on_page_load = on_event;

        let webview = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(to_wry_rect(bounds))
            .with_devtools(cfg!(debug_assertions))
            .with_back_forward_navigation_gestures(true)
            .with_initialization_script(FOCUS_SCRIPT)
            .with_initialization_script(CONSOLE_CAPTURE_SCRIPT)
            .with_ipc_handler(move |request| {
                let body = request.body();
                if body == PAGE_FOCUSED_MESSAGE {
                    on_ipc(WebViewEvent::PageFocused);
                } else if body == ANNOTATE_EXITED_MESSAGE {
                    on_ipc(WebViewEvent::AnnotateExited);
                } else if let Some(json) = body.strip_prefix(ANNOTATION_MESSAGE_PREFIX) {
                    on_ipc(WebViewEvent::Annotation(json.to_owned()));
                } else if let Some(action) = body.strip_prefix(PAGE_ACTION_PREFIX) {
                    on_ipc(WebViewEvent::PageAction(action.to_owned()));
                }
            })
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

    pub fn can_go_back(&self) -> Result<bool, Error> {
        Ok(self.webview.can_go_back()?)
    }

    pub fn can_go_forward(&self) -> Result<bool, Error> {
        Ok(self.webview.can_go_forward()?)
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

    /// Runs `script` in the page and calls `on_result` with its value serialized as JSON.
    pub fn evaluate(
        &self,
        script: &str,
        on_result: impl FnOnce(String) + Send + 'static,
    ) -> Result<(), Error> {
        let on_result = Mutex::new(Some(on_result));
        Ok(self
            .webview
            .evaluate_script_with_callback(script, move |result| {
                if let Some(on_result) = on_result.lock().ok().and_then(|mut slot| slot.take()) {
                    on_result(result);
                }
            })?)
    }

    /// Captures what the web view shows as a PNG and calls `on_done` with it, or with `None` if
    /// WebKit could not take the snapshot.
    pub fn snapshot_png(&self, on_done: impl FnOnce(Option<Vec<u8>>) + 'static) {
        let on_done = Cell::new(Some(on_done));
        let handler = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
            if let Some(on_done) = on_done.take() {
                // SAFETY: WebKit passes either null or a valid image for the duration of the call.
                on_done(unsafe { image.as_ref() }.and_then(png_data));
            }
        });
        // SAFETY: a `None` configuration snapshots the visible bounds, and `handler` matches the
        // completion handler's signature.
        unsafe {
            self.webview
                .webview()
                .takeSnapshotWithConfiguration_completionHandler(None, &handler);
        }
    }

    /// Clicks at `x`, `y` (logical pixels from the web view's top-left corner) with native mouse
    /// events, which pages cannot tell from the user's own. The user's pointer does not move.
    /// Returns whether the events were sent, which needs the web view to be in a window.
    pub fn click_at(&self, x: f32, y: f32) -> bool {
        let view = self.webview.webview();
        let Some(window) = view.window() else {
            return false;
        };
        let y = if view.isFlipped() {
            y
        } else {
            view.bounds().size.height as f32 - y
        };
        let location = view.convertPoint_toView(NSPoint::new(x.into(), y.into()), None);
        let mouse_event = |event_type| {
            NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                event_type,
                location,
                NSEventModifierFlags::empty(),
                NSProcessInfo::processInfo().systemUptime(),
                window.windowNumber(),
                None,
                0,
                1,
                1.,
            )
        };
        let (Some(moved), Some(down), Some(up)) = (
            mouse_event(NSEventType::MouseMoved),
            mouse_event(NSEventType::LeftMouseDown),
            mouse_event(NSEventType::LeftMouseUp),
        ) else {
            return false;
        };
        view.mouseMoved(&moved);
        view.mouseDown(&down);
        view.mouseUp(&up);
        true
    }

    /// Types `text`, then presses Return when `submit`, with native key events into the page's
    /// focused element. Keyboard focus moves to the page while typing and then goes back. Returns
    /// whether the events were sent, which needs the web view to be in the key window.
    pub fn type_text(&self, text: &str, submit: bool) -> bool {
        let view = self.webview.webview();
        let Some(window) = view.window().filter(|window| window.isKeyWindow()) else {
            return false;
        };
        let key_event = |event_type, characters: &NSString, key_code| {
            NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                event_type,
                NSPoint::ZERO,
                NSEventModifierFlags::empty(),
                NSProcessInfo::processInfo().systemUptime(),
                window.windowNumber(),
                None,
                characters,
                characters,
                false,
                key_code,
            )
        };
        let press = |characters: &str, key_code| {
            let characters = NSString::from_str(characters);
            if let (Some(down), Some(up)) = (
                key_event(NSEventType::KeyDown, &characters, key_code),
                key_event(NSEventType::KeyUp, &characters, key_code),
            ) {
                view.keyDown(&down);
                view.keyUp(&up);
            }
        };

        let previous_responder = window.firstResponder();
        window.makeFirstResponder(Some(&view));
        let mut buffer = [0; 4];
        for character in text.chars() {
            press(character.encode_utf8(&mut buffer), 0);
        }
        if submit {
            press("\r", RETURN_KEY_CODE);
        }
        window.makeFirstResponder(previous_responder.as_deref());
        true
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

fn png_data(image: &NSImage) -> Option<Vec<u8>> {
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: an empty properties dictionary is valid for PNG encoding.
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
}

fn to_wry_rect(bounds: RectF) -> Rect {
    Rect {
        position: LogicalPosition::new(bounds.origin_x(), bounds.origin_y()).into(),
        size: LogicalSize::new(bounds.width(), bounds.height()).into(),
    }
}
