use pathfinder_geometry::rect::RectF;
use warpui::ZoomFactor;
use warpui::zoom::Scale;

/// Converts the rect of a browser pane's content area, as laid out by WarpUI, to the web view's
/// bounds in window coordinates. WarpUI implements zoom by laying out against a window shrunk by
/// the zoom factor, while the web view is a native view positioned in unzoomed coordinates.
pub(super) fn webview_bounds(content_rect: RectF, zoom: ZoomFactor) -> RectF {
    RectF::new(
        content_rect.origin().scale_up(zoom),
        content_rect.size().scale_up(zoom),
    )
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
