use computer_use::{Action, Key, MouseButton, ScrollDirection, ScrollDistance};
use pathfinder_geometry::vector::{Vector2I, vec2f};

use super::actions;
use crate::input::{self, InputEvent, Modifiers, NamedKey, PointerButton};

#[test]
fn maps_picture_fractions_to_window_pixels() {
    let actions = actions(
        &InputEvent::Down {
            button: PointerButton::Right,
            at: vec2f(0.5, 0.25),
            modifiers: Modifiers::default(),
        },
        vec2f(2000., 1200.),
    );
    assert_eq!(
        actions,
        vec![Action::MouseDown {
            button: MouseButton::Right,
            at: Vector2I::new(1000, 300),
        }]
    );
}

#[test]
fn releases_where_the_pointer_was_let_go() {
    let actions = actions(
        &InputEvent::Up {
            button: PointerButton::Left,
            at: vec2f(1.2, 0.5),
        },
        vec2f(100., 100.),
    );
    assert_eq!(
        actions,
        vec![
            Action::MouseMove {
                to: Vector2I::new(100, 50)
            },
            Action::MouseUp {
                button: MouseButton::Left
            },
        ]
    );
}

#[test]
fn scrolls_each_axis_that_moved() {
    let actions = actions(
        &InputEvent::Scroll {
            at: vec2f(0., 0.),
            delta: vec2f(-30., 12.4),
        },
        vec2f(100., 100.),
    );
    assert_eq!(
        actions,
        vec![
            Action::MouseWheel {
                at: Vector2I::new(0, 0),
                direction: ScrollDirection::Down,
                distance: ScrollDistance::Pixels(12),
            },
            Action::MouseWheel {
                at: Vector2I::new(0, 0),
                direction: ScrollDirection::Left,
                distance: ScrollDistance::Pixels(30),
            },
        ]
    );
}

#[test]
fn holds_modifiers_around_a_key_press() {
    let actions = actions(
        &InputEvent::Key {
            key: input::Key::Named(NamedKey::Enter),
            modifiers: Modifiers {
                cmd: true,
                shift: true,
                ..Default::default()
            },
        },
        vec2f(100., 100.),
    );
    assert_eq!(
        actions,
        vec![
            Action::KeyDown {
                key: Key::Keycode(55)
            },
            Action::KeyDown {
                key: Key::Keycode(56)
            },
            Action::KeyDown {
                key: Key::Keycode(36)
            },
            Action::KeyUp {
                key: Key::Keycode(36)
            },
            Action::KeyUp {
                key: Key::Keycode(56)
            },
            Action::KeyUp {
                key: Key::Keycode(55)
            },
        ]
    );
}
