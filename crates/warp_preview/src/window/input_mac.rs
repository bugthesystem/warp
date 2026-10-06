//! Delivers input to another app's window with `computer_use`, which posts events to the window's
//! process so the window is not raised and the user's pointer does not move.

use std::sync::mpsc;

use computer_use::{
    Action, Key as ComputerKey, MouseButton, Options, ScrollDirection, ScrollDistance, Target,
    TargetedAction,
};
use pathfinder_geometry::vector::{Vector2F, Vector2I};

use crate::input::{InputEvent, Key, Modifiers, PointerButton};

/// A thread that owns a `computer_use` actor for one window. The actor keeps state between
/// events, such as a held button during a drag, so one worker serves the window for its life.
pub(super) struct InputWorker {
    actions: mpsc::Sender<Vec<Action>>,
}

impl InputWorker {
    pub(super) fn start(window_id: u32, pid: i32) -> Option<Self> {
        let (actions, receiver) = mpsc::channel::<Vec<Action>>();
        let owner = format!("warp-preview-{window_id}");
        std::thread::Builder::new()
            .name("preview-input".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        log::warn!("Couldn't start preview input: {err}");
                        return;
                    }
                };
                let mut actor = computer_use::create_actor();
                actor.set_background_session_owner(Some(owner.clone()));
                let target = Target::Window { window_id, pid };
                while let Ok(batch) = receiver.recv() {
                    let batch: Vec<TargetedAction> = batch
                        .into_iter()
                        .map(|action| TargetedAction { action, target })
                        .collect();
                    let options = Options {
                        screenshot_params: None,
                        background_enabled: true,
                        pointer_sink: None,
                    };
                    if let Err(err) = runtime.block_on(actor.perform_actions(&batch, options)) {
                        log::warn!("Preview input failed: {err}");
                    }
                }
                computer_use::end_background_session(&owner);
            })
            .ok()?;
        Some(Self { actions })
    }

    pub(super) fn send(&self, actions: Vec<Action>) {
        if !actions.is_empty() {
            let _ = self.actions.send(actions);
        }
    }
}

/// The `computer_use` actions for `event` on a window whose captured picture is `size` pixels.
pub(super) fn actions(event: &InputEvent, size: Vector2F) -> Vec<Action> {
    let to_window = |at: Vector2F| {
        let at = at.max(Vector2F::zero()).min(Vector2F::splat(1.)) * size;
        Vector2I::new(at.x().round() as i32, at.y().round() as i32)
    };
    match event {
        InputEvent::Move { at } | InputEvent::Drag { at } => {
            vec![Action::MouseMove { to: to_window(*at) }]
        }
        InputEvent::Down {
            button,
            at,
            modifiers,
        } => with_modifiers(
            *modifiers,
            vec![Action::MouseDown {
                button: mouse_button(*button),
                at: to_window(*at),
            }],
        ),
        InputEvent::Up { button, at } => vec![
            Action::MouseMove { to: to_window(*at) },
            Action::MouseUp {
                button: mouse_button(*button),
            },
        ],
        InputEvent::Scroll { at, delta } => {
            let at = to_window(*at);
            let mut actions = Vec::new();
            for (amount, positive, negative) in [
                (delta.y(), ScrollDirection::Down, ScrollDirection::Up),
                (delta.x(), ScrollDirection::Right, ScrollDirection::Left),
            ] {
                let pixels = amount.abs().round() as i32;
                if pixels == 0 {
                    continue;
                }
                actions.push(Action::MouseWheel {
                    at,
                    direction: if amount > 0. { positive } else { negative },
                    distance: ScrollDistance::Pixels(pixels),
                });
            }
            actions
        }
        InputEvent::Key { key, modifiers } => {
            let key = match key {
                Key::Named(named) => ComputerKey::Keycode(i32::from(named.mac_keycode())),
                Key::Char(char) => ComputerKey::Char(*char),
            };
            with_modifiers(
                *modifiers,
                vec![Action::KeyDown { key: key.clone() }, Action::KeyUp { key }],
            )
        }
        InputEvent::Text(text) => vec![Action::TypeText { text: text.clone() }],
    }
}

/// Wraps `actions` in presses and releases of `modifiers`.
fn with_modifiers(modifiers: Modifiers, actions: Vec<Action>) -> Vec<Action> {
    let keys: Vec<ComputerKey> = modifiers
        .mac_keycodes()
        .into_iter()
        .map(|keycode| ComputerKey::Keycode(i32::from(keycode)))
        .collect();
    let mut wrapped: Vec<Action> = keys
        .iter()
        .map(|key| Action::KeyDown { key: key.clone() })
        .collect();
    wrapped.extend(actions);
    wrapped.extend(keys.into_iter().rev().map(|key| Action::KeyUp { key }));
    wrapped
}

fn mouse_button(button: PointerButton) -> MouseButton {
    match button {
        PointerButton::Left => MouseButton::Left,
        PointerButton::Right => MouseButton::Right,
        PointerButton::Middle => MouseButton::Middle,
    }
}

#[cfg(test)]
#[path = "input_mac_tests.rs"]
mod tests;
