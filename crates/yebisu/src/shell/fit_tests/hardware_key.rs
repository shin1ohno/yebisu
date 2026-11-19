//! Review 0a57da3 G1/G2: only a press a hardware keyboard sends counts as
//! one. The software keyboard's own presses (herdr-uikit `tap_key`) never do.
use super::super::*;
use gpui::{Keystroke, Modifiers};

fn key(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke {
        modifiers,
        key: key.to_owned(),
        key_char: key_char.map(str::to_owned),
    }
}

#[test]
fn software_keyboard_presses_are_not_hardware_keys() {
    let none = Modifiers::default();
    let control = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    // keymap::control_keystroke and keymap::backspace (herdr-uikit 99d7928).
    for press in [
        key("enter", Some("\n"), none),
        key("tab", Some("\t"), none),
        key("escape", None, none),
        key("backspace", None, none),
        key("a", None, control),
        key("z", None, control),
    ] {
        assert!(!is_hardware_key(&press), "{press:?}");
    }
    // Shortcuts and the edit menu's presses are not proof either.
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    assert!(!is_hardware_key(&key("c", Some("c"), command)));
    assert!(!is_hardware_key(&key("left", None, none)));
}

#[test]
fn printable_presses_are_hardware_keys() {
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    let option = Modifiers {
        alt: true,
        ..Modifiers::default()
    };
    assert!(is_hardware_key(&key("a", Some("a"), Modifiers::default())));
    assert!(is_hardware_key(&key("a", Some("A"), shift)));
    assert!(is_hardware_key(&key("e", Some("´"), option)));
    assert!(is_hardware_key(&key(
        "space",
        Some(" "),
        Modifiers::default()
    )));
}
