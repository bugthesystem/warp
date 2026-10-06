use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warp_preview::input::{InputEvent, Key, Modifiers, NamedKey, PointerButton};
use warpui::keymap::Keystroke;

use super::PlayingInput;
use crate::preview::stage::StageInput;

/// A 200x100 frame drawn in a 200x200 area at the window's origin: the picture is the middle half.
const FRAME: (u32, u32) = (200, 100);

fn area() -> RectF {
    RectF::new(vec2f(0., 0.), vec2f(200., 200.))
}

fn down(at: (i32, i32), button: PointerButton) -> StageInput {
    StageInput::Down {
        at,
        button,
        modifiers: Modifiers::default(),
    }
}

#[test]
fn presses_and_releases_the_left_button_through_a_drag() {
    let mut input = PlayingInput::default();
    assert_eq!(
        input.events(&down((100, 100), PointerButton::Left), FRAME, area()),
        vec![InputEvent::Down {
            button: PointerButton::Left,
            at: vec2f(0.5, 0.5),
            modifiers: Modifiers::default(),
        }]
    );
    assert_eq!(
        input.events(&StageInput::Drag { at: (300, 100) }, FRAME, area()),
        vec![InputEvent::Drag { at: vec2f(1., 0.5) }],
        "drags past the picture's edge are clamped to it"
    );
    assert_eq!(
        input.events(&StageInput::Up { at: (150, 100) }, FRAME, area()),
        vec![InputEvent::Up {
            button: PointerButton::Left,
            at: vec2f(0.75, 0.5),
        }]
    );
    assert!(
        input
            .events(&StageInput::Up { at: (150, 100) }, FRAME, area())
            .is_empty()
    );
}

#[test]
fn ignores_presses_on_the_letterbox_bars() {
    let mut input = PlayingInput::default();
    assert!(
        input
            .events(&down((100, 10), PointerButton::Left), FRAME, area())
            .is_empty()
    );
    assert!(
        input
            .events(&StageInput::Drag { at: (100, 100) }, FRAME, area())
            .is_empty()
    );
}

#[test]
fn clicks_with_the_right_and_middle_buttons() {
    let mut input = PlayingInput::default();
    let events = input.events(&down((0, 50), PointerButton::Right), FRAME, area());
    assert_eq!(
        events,
        vec![
            InputEvent::Down {
                button: PointerButton::Right,
                at: vec2f(0., 0.),
                modifiers: Modifiers::default(),
            },
            InputEvent::Up {
                button: PointerButton::Right,
                at: vec2f(0., 0.),
            },
        ]
    );
}

#[test]
fn releases_a_drag_whose_release_happened_off_the_picture() {
    let mut input = PlayingInput::default();
    input.events(&down((100, 100), PointerButton::Left), FRAME, area());
    assert_eq!(
        input.events(&StageInput::Hover { at: (50, 75) }, FRAME, area()),
        vec![InputEvent::Up {
            button: PointerButton::Left,
            at: vec2f(0.25, 0.25),
        }]
    );
    assert!(
        input
            .events(&StageInput::Hover { at: (60, 75) }, FRAME, area())
            .is_empty()
    );
}

#[test]
fn scrolls_where_the_pointer_last_was() {
    let mut input = PlayingInput::default();
    assert!(
        input
            .events(&StageInput::Scroll { delta: (0, -10) }, FRAME, area())
            .is_empty(),
        "nowhere to scroll before the pointer is over the picture"
    );
    input.events(&StageInput::Hover { at: (50, 100) }, FRAME, area());
    assert_eq!(
        input.events(&StageInput::Scroll { delta: (0, -10) }, FRAME, area()),
        vec![InputEvent::Scroll {
            at: vec2f(0.25, 0.5),
            delta: vec2f(0., 10.),
        }]
    );
}

#[test]
fn forwards_keys_with_their_modifiers() {
    let mut input = PlayingInput::default();
    let keystroke = |key: &str, shift: bool, cmd: bool| {
        StageInput::Key(Keystroke {
            key: key.to_owned(),
            shift,
            cmd,
            ..Default::default()
        })
    };
    assert_eq!(
        input.events(&keystroke("a", true, false), FRAME, area()),
        vec![InputEvent::Key {
            key: Key::Char('A'),
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
        }]
    );
    assert_eq!(
        input.events(&keystroke("enter", false, true), FRAME, area()),
        vec![InputEvent::Key {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers {
                cmd: true,
                ..Default::default()
            },
        }]
    );
    assert!(
        input
            .events(&keystroke("numlock", false, false), FRAME, area())
            .is_empty()
    );
}
