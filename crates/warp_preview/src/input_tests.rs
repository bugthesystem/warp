use super::{Key, Modifiers, NamedKey};

#[test]
fn parses_named_keys_and_function_keys() {
    assert_eq!(NamedKey::parse("Enter"), Some(NamedKey::Enter));
    assert_eq!(NamedKey::parse("return"), Some(NamedKey::Enter));
    assert_eq!(NamedKey::parse("pagedown"), Some(NamedKey::PageDown));
    assert_eq!(NamedKey::parse("f5"), Some(NamedKey::F(5)));
    assert_eq!(NamedKey::parse("f13"), None);
    assert_eq!(NamedKey::parse("f"), None);
    assert_eq!(NamedKey::parse("hyper"), None);
}

#[test]
fn parses_chords() {
    assert_eq!(
        Key::parse_chord("cmd+shift+k"),
        Some((
            Key::Char('k'),
            Modifiers {
                cmd: true,
                shift: true,
                ..Default::default()
            }
        ))
    );
    assert_eq!(
        Key::parse_chord("enter"),
        Some((Key::Named(NamedKey::Enter), Modifiers::default()))
    );
    assert_eq!(
        Key::parse_chord("+"),
        None,
        "a lone plus splits into two empty parts"
    );
    assert_eq!(Key::parse_chord("hyper+k"), None);
    assert_eq!(Key::parse_chord("cmd+"), None);
}

#[test]
fn maps_keys_to_mac_keycodes() {
    assert_eq!(NamedKey::Enter.mac_keycode(), 36);
    assert_eq!(NamedKey::Up.mac_keycode(), 126);
    assert_eq!(NamedKey::F(1).mac_keycode(), 122);
    assert_eq!(NamedKey::F(12).mac_keycode(), 111);
}

#[test]
fn presses_modifiers_in_a_fixed_order() {
    let modifiers = Modifiers {
        cmd: true,
        shift: false,
        alt: true,
        ctrl: true,
    };
    assert_eq!(modifiers.mac_keycodes(), vec![55, 58, 59]);
    assert!(modifiers.any());
    assert!(!Modifiers::default().any());
}

#[test]
fn reads_warpui_key_names() {
    assert_eq!(Key::from_name("a"), Some(Key::Char('a')));
    assert_eq!(Key::from_name(" "), Some(Key::Named(NamedKey::Space)));
    assert_eq!(Key::from_name("escape"), Some(Key::Named(NamedKey::Escape)));
    assert_eq!(Key::from_name("numlock"), None);
}
