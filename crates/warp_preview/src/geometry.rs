//! Fitting a source's picture into an area of the screen, and mapping points between the two.

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};

/// The largest rect with the aspect ratio of `content` that fits centered in `area`, leaving
/// letterbox bars on two sides. Returns `area` when `content` has no size.
pub fn letterbox(content: Vector2F, area: RectF) -> RectF {
    if content.x() <= 0. || content.y() <= 0. {
        return area;
    }
    let scale = (area.width() / content.x()).min(area.height() / content.y());
    let size = content * scale;
    let origin = area.origin() + (area.size() - size) * 0.5;
    RectF::new(origin, size)
}

/// Maps `point`, in the same space as `area`, to the source's own coordinates, where the source
/// is `content` in size and drawn letterboxed in `area`. `None` for points on the bars.
pub fn to_source(point: Vector2F, content: Vector2F, area: RectF) -> Option<Vector2F> {
    let picture = letterbox(content, area);
    if picture.width() <= 0. || picture.height() <= 0. || !picture.contains_point(point) {
        return None;
    }
    let relative = point - picture.origin();
    Some(vec2f(
        relative.x() * content.x() / picture.width(),
        relative.y() * content.y() / picture.height(),
    ))
}

/// The size to capture a source of `size` at so that neither side exceeds `max_side`, keeping its
/// aspect ratio. Sizes already within the limit are returned unchanged.
pub fn scale_within(size: (u32, u32), max_side: u32) -> (u32, u32) {
    let (width, height) = size;
    let longest = width.max(height);
    if longest <= max_side || longest == 0 {
        return size;
    }
    let scale = max_side as f64 / longest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
