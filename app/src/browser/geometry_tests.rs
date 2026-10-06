use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warpui::ZoomFactor;

use super::webview_bounds;

#[test]
fn bounds_match_content_rect_at_default_zoom() {
    let content_rect = RectF::new(vec2f(120., 40.), vec2f(800., 600.));

    let bounds = webview_bounds(content_rect, ZoomFactor::default());

    assert_eq!(bounds, content_rect);
}

#[test]
fn bounds_scale_origin_and_size_by_zoom() {
    let content_rect = RectF::new(vec2f(100., 40.), vec2f(800., 600.));

    let bounds = webview_bounds(content_rect, ZoomFactor::new(1.5));

    assert_eq!(bounds, RectF::new(vec2f(150., 60.), vec2f(1200., 900.)));
}
