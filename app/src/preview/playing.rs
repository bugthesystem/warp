//! Turns the user's input over a playing preview's picture into input for the preview's source.

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warp_preview::Source;
use warp_preview::geometry::to_fraction;
use warp_preview::input::{InputEvent, Key, Modifiers, PointerButton};
use warpui::{SingletonEntity, View, ViewContext};

use super::stage::StageInput;
use super::streams::{PreviewId, PreviewStreams};

/// Shown when Playing needs the Accessibility permission. macOS shows its own prompt alongside.
pub(super) const NEEDS_ACCESSIBILITY: &str = "Allow Warp in Accessibility to play windows";
/// Shown when the window to play is minimized or on another desktop, where input can't reach it.
pub(super) const NOT_ON_THIS_DESKTOP: &str = "Move the window to this desktop to play it";

/// Switches `front` between Watching and Playing. Asks for the Accessibility permission a window
/// needs to be played, and returns the notice to show while it is missing.
pub(super) fn set_playing<V: View>(
    front: PreviewId,
    playing: bool,
    ctx: &mut ViewContext<V>,
) -> Result<(), &'static str> {
    let window_id =
        PreviewStreams::as_ref(ctx)
            .get(front)
            .and_then(|preview| match &preview.source {
                Source::Window(window) => Some(window.window_id),
                Source::Browser { .. } => None,
            });
    if let (true, Some(window_id)) = (playing, window_id) {
        if !warp_preview::window::has_input_permission() {
            warp_preview::window::request_input_permission();
            return Err(NEEDS_ACCESSIBILITY);
        }
        if !warp_preview::window::is_on_current_desktop(window_id) {
            return Err(NOT_ON_THIS_DESKTOP);
        }
    }
    PreviewStreams::handle(ctx)
        .update(ctx, |streams, ctx| streams.set_playing(front, playing, ctx));
    if playing {
        ctx.focus_self();
    }
    Ok(())
}

/// What a view remembers between input events on its playing preview.
#[derive(Default)]
pub(super) struct PlayingInput {
    /// Where the pointer last was over the picture, as a fraction of it. Scrolls happen there.
    hover: Option<Vector2F>,
    /// Whether the left button is held after a press on the picture.
    dragging: bool,
}

impl PlayingInput {
    /// Delivers `input` on the stage saved at `position_id` to `front`, if it is playing.
    pub(super) fn forward<V: View>(
        &mut self,
        front: PreviewId,
        input: &StageInput,
        position_id: &str,
        ctx: &mut ViewContext<V>,
    ) {
        if matches!(input, StageInput::Down { .. }) {
            ctx.focus_self();
        }
        let Some(area) = ctx.element_position_by_id_at_last_frame(ctx.window_id(), position_id)
        else {
            return;
        };
        let streams = PreviewStreams::as_ref(ctx);
        let Some(frame_size) = streams
            .get(front)
            .filter(|preview| preview.playing)
            .and_then(|preview| preview.frame_size)
        else {
            return;
        };
        for event in self.events(input, frame_size, area) {
            streams.send_input(front, event);
        }
    }

    /// The source input for `input` on a picture of `frame_size` pixels drawn letterboxed in
    /// `area`, which is in window points like `input`'s positions.
    pub(super) fn events(
        &mut self,
        input: &StageInput,
        frame_size: (u32, u32),
        area: RectF,
    ) -> Vec<InputEvent> {
        let content = vec2f(frame_size.0 as f32, frame_size.1 as f32);
        let fraction = |(x, y): (i32, i32), clamp: bool| {
            to_fraction(vec2f(x as f32, y as f32), content, area, clamp)
        };
        match input {
            StageInput::Down {
                at,
                button,
                modifiers,
            } => {
                let Some(at) = fraction(*at, false) else {
                    return Vec::new();
                };
                self.hover = Some(at);
                let mut events = self.release_lost_drag(at);
                events.push(InputEvent::Down {
                    button: *button,
                    at,
                    modifiers: *modifiers,
                });
                // Only the left button reports drags and releases, so the others are clicks.
                match button {
                    PointerButton::Left => self.dragging = true,
                    PointerButton::Right | PointerButton::Middle => events.push(InputEvent::Up {
                        button: *button,
                        at,
                    }),
                }
                events
            }
            StageInput::Up { at } => {
                if !self.dragging {
                    return Vec::new();
                }
                self.dragging = false;
                fraction(*at, true)
                    .map(|at| InputEvent::Up {
                        button: PointerButton::Left,
                        at,
                    })
                    .into_iter()
                    .collect()
            }
            StageInput::Drag { at } => {
                if !self.dragging {
                    return Vec::new();
                }
                let at = fraction(*at, true);
                self.hover = at;
                at.map(|at| InputEvent::Drag { at }).into_iter().collect()
            }
            StageInput::Hover { at } => {
                let at = fraction(*at, false);
                // A move with no button held after a press means the release happened somewhere
                // the picture didn't see.
                let events = match at.or(self.hover) {
                    Some(last) => self.release_lost_drag(last),
                    None => Vec::new(),
                };
                self.hover = at;
                events
            }
            StageInput::Scroll { delta } => match self.hover {
                // AppKit reports a scroll that reveals content further down as negative.
                Some(at) => vec![InputEvent::Scroll {
                    at,
                    delta: vec2f(-delta.0 as f32, -delta.1 as f32),
                }],
                None => Vec::new(),
            },
            StageInput::Key(keystroke) => {
                let Some(mut key) = Key::from_name(&keystroke.key) else {
                    return Vec::new();
                };
                if let Key::Char(char) = key
                    && keystroke.shift
                {
                    key = Key::Char(char.to_ascii_uppercase());
                }
                vec![InputEvent::Key {
                    key,
                    modifiers: Modifiers {
                        cmd: keystroke.cmd,
                        shift: keystroke.shift,
                        alt: keystroke.alt,
                        ctrl: keystroke.ctrl,
                    },
                }]
            }
        }
    }

    /// Releases the left button at `at` if a drag is still open.
    fn release_lost_drag(&mut self, at: Vector2F) -> Vec<InputEvent> {
        if !std::mem::take(&mut self.dragging) {
            return Vec::new();
        }
        vec![InputEvent::Up {
            button: PointerButton::Left,
            at,
        }]
    }
}

#[cfg(test)]
#[path = "playing_tests.rs"]
mod tests;
