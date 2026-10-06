use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;

use super::PipCorner;

#[test]
fn snaps_to_the_nearest_corner() {
    let window = vec2f(1000., 800.);
    let at = |x, y| RectF::new(vec2f(x, y), vec2f(100., 60.));
    assert_eq!(PipCorner::nearest(at(10., 10.), window), PipCorner::TopLeft);
    assert_eq!(PipCorner::nearest(at(850., 20.), window), PipCorner::TopRight);
    assert_eq!(PipCorner::nearest(at(30., 700.), window), PipCorner::BottomLeft);
    assert_eq!(PipCorner::nearest(at(880., 720.), window), PipCorner::BottomRight);
}
