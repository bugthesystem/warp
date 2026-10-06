//! Pointer and keyboard input for a preview's source, from the user in Playing or from an agent.
//!
//! Positions are fractions of the picture (0 to 1 on each axis from its top left), so callers need
//! not know the source's own coordinates; each source maps them onto its window or page.

use pathfinder_geometry::vector::Vector2F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// A key that is not a printable character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    Backspace,
    Escape,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    /// A function key, F1 to F12.
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Named(NamedKey),
    Char(char),
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// The pointer moved with no button held.
    Move {
        at: Vector2F,
    },
    /// The pointer moved while a button is held.
    Drag {
        at: Vector2F,
    },
    Down {
        button: PointerButton,
        at: Vector2F,
        modifiers: Modifiers,
    },
    Up {
        button: PointerButton,
        at: Vector2F,
    },
    /// Scrolls by `delta` picture points; positive `y` scrolls the content down, as a wheel turned
    /// towards the user does.
    Scroll {
        at: Vector2F,
        delta: Vector2F,
    },
    /// Presses and releases `key` with `modifiers` held.
    Key {
        key: Key,
        modifiers: Modifiers,
    },
    /// Types text as if each character were pressed.
    Text(String),
}

impl NamedKey {
    /// Parses a key name such as `enter`, `pagedown` or `f5`, ignoring case.
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        Some(match name.as_str() {
            "enter" | "return" => Self::Enter,
            "tab" => Self::Tab,
            "space" => Self::Space,
            "backspace" => Self::Backspace,
            "escape" | "esc" => Self::Escape,
            "delete" => Self::Delete,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            "left" => Self::Left,
            "right" => Self::Right,
            "up" => Self::Up,
            "down" => Self::Down,
            other => {
                let number: u8 = other.strip_prefix('f')?.parse().ok()?;
                if !(1..=12).contains(&number) {
                    return None;
                }
                Self::F(number)
            }
        })
    }

    /// The macOS virtual key code.
    pub fn mac_keycode(self) -> u16 {
        match self {
            Self::Enter => 36,
            Self::Tab => 48,
            Self::Space => 49,
            Self::Backspace => 51,
            Self::Escape => 53,
            Self::Delete => 117,
            Self::Home => 115,
            Self::End => 119,
            Self::PageUp => 116,
            Self::PageDown => 121,
            Self::Left => 123,
            Self::Right => 124,
            Self::Down => 125,
            Self::Up => 126,
            Self::F(number) => {
                const F_KEYS: [u16; 12] = [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111];
                F_KEYS[usize::from(number.clamp(1, 12) - 1)]
            }
        }
    }
}

impl Key {
    /// The key for a WarpUI-style key name: a single character, `" "` for space, or a name such as
    /// `enter` or `f5`.
    pub fn from_name(name: &str) -> Option<Key> {
        if name == " " {
            return Some(Key::Named(NamedKey::Space));
        }
        let mut chars = name.chars();
        match (chars.next(), chars.next()) {
            (Some(only), None) => Some(Key::Char(only)),
            _ => NamedKey::parse(name).map(Key::Named),
        }
    }

    /// Parses a chord such as `cmd+shift+k`, `enter` or `a`. Modifier names are `cmd`, `shift`,
    /// `alt` (or `option`) and `ctrl`.
    pub fn parse_chord(chord: &str) -> Option<(Key, Modifiers)> {
        let mut modifiers = Modifiers::default();
        let mut parts = chord.split('+').peekable();
        let mut key = None;
        while let Some(part) = parts.next() {
            let is_last = parts.peek().is_none();
            if !is_last {
                match part.to_ascii_lowercase().as_str() {
                    "cmd" | "command" | "meta" => modifiers.cmd = true,
                    "shift" => modifiers.shift = true,
                    "alt" | "option" => modifiers.alt = true,
                    "ctrl" | "control" => modifiers.ctrl = true,
                    _ => return None,
                }
                continue;
            }
            let mut chars = part.chars();
            key = match (chars.next(), chars.next()) {
                (Some(only), None) => Some(Key::Char(only)),
                _ => NamedKey::parse(part).map(Key::Named),
            };
        }
        key.map(|key| (key, modifiers))
    }
}

impl Modifiers {
    pub fn any(self) -> bool {
        self.cmd || self.shift || self.alt || self.ctrl
    }

    /// macOS virtual key codes of the held modifiers, in the order they are pressed.
    pub fn mac_keycodes(self) -> Vec<u16> {
        [
            (self.cmd, 55),
            (self.shift, 56),
            (self.alt, 58),
            (self.ctrl, 59),
        ]
        .into_iter()
        .filter_map(|(held, keycode)| held.then_some(keycode))
        .collect()
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
