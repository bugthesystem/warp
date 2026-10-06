use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;

use super::*;

#[test]
fn letterbox_pillarboxes_tall_content() {
    let area = RectF::new(vec2f(10., 20.), vec2f(400., 200.));
    let picture = letterbox(vec2f(100., 100.), area);
    assert_eq!(picture, RectF::new(vec2f(110., 20.), vec2f(200., 200.)));
}

#[test]
fn letterbox_adds_bars_above_and_below_wide_content() {
    let area = RectF::new(vec2f(0., 0.), vec2f(200., 200.));
    let picture = letterbox(vec2f(400., 100.), area);
    assert_eq!(picture, RectF::new(vec2f(0., 75.), vec2f(200., 50.)));
}

#[test]
fn letterbox_keeps_area_for_empty_content() {
    let area = RectF::new(vec2f(5., 5.), vec2f(50., 60.));
    assert_eq!(letterbox(vec2f(0., 10.), area), area);
}

#[test]
fn to_source_maps_through_the_letterbox() {
    let area = RectF::new(vec2f(10., 20.), vec2f(400., 200.));
    let content = vec2f(1000., 1000.);
    assert_eq!(
        to_source(vec2f(110., 20.), content, area),
        Some(vec2f(0., 0.))
    );
    assert_eq!(
        to_source(vec2f(210., 120.), content, area),
        Some(vec2f(500., 500.))
    );
}

#[test]
fn to_source_ignores_points_on_the_bars() {
    let area = RectF::new(vec2f(10., 20.), vec2f(400., 200.));
    assert_eq!(to_source(vec2f(50., 100.), vec2f(100., 100.), area), None);
}

#[test]
fn scale_within_shrinks_the_longest_side() {
    assert_eq!(scale_within((3200, 1800), 1600), (1600, 900));
    assert_eq!(scale_within((900, 1600), 320), (180, 320));
}

#[test]
fn scale_within_keeps_small_sizes() {
    assert_eq!(scale_within((800, 600), 1600), (800, 600));
    assert_eq!(scale_within((0, 0), 10), (0, 0));
}

#[test]
fn to_fraction_measures_from_the_picture_not_the_bars() {
    let area = RectF::new(vec2f(0., 0.), vec2f(200., 200.));
    let content = vec2f(400., 200.);
    // The picture is 200x100, centered with bars above and below.
    assert_eq!(
        to_fraction(vec2f(100., 75.), content, area, false),
        Some(vec2f(0.5, 0.25))
    );
    assert_eq!(to_fraction(vec2f(100., 10.), content, area, false), None);
    assert_eq!(
        to_fraction(vec2f(250., 10.), content, area, true),
        Some(vec2f(1., 0.))
    );
}
