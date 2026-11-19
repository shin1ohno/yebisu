//! One routing rule for every key the focused filter field receives (PR 77
//! reviews 0df6 SF1 and 9d5a S1), one test per row: printable and
//! Option-composed text go into the field and never to the terminal; Return
//! and Escape leave the field; Command shortcuts go on to the app; Control,
//! arrows, Tab and Fn give the keys back and that key reaches the terminal.
use super::screen::{Receipts, field_focused, query, terminal, type_into_field};
use crate::{
    app::Smoke,
    shell::filter::{FilterEdit, filter_edit},
};
use gpui::{Entity, Keystroke, Modifiers, TestAppContext, VisualTestContext};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::time::Duration;

/// The pane input events the terminal sent since the last call.
fn sent(view: &Entity<Smoke>, receipts: &Receipts, cx: &mut VisualTestContext) -> Vec<String> {
    for _ in 0..10 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut events = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let ClientMessage::ClientShellPaneInput { events: input, .. } = message {
            events.extend(
                input
                    .iter()
                    .map(|event: &ClientPaneInputEvent| format!("{event:?}")),
            );
        }
    }
    events
}

fn key(text: &'static str) -> Keystroke {
    Keystroke::parse(text)
        .ok()
        .unwrap_or_else(|| panic!("not a keystroke: {text}"))
}

/// A press as the platform sends it: the key and the text it typed.
fn typed(modifiers: Modifiers, key: &str, key_char: Option<&str>) -> Keystroke {
    Keystroke {
        modifiers,
        key: key.into(),
        key_char: key_char.map(Into::into),
    }
}

fn alt(key: &str, key_char: Option<&str>) -> Keystroke {
    let alt = Modifiers {
        alt: true,
        ..Modifiers::default()
    };
    typed(alt, key, key_char)
}

#[test]
fn every_row_of_the_rule() {
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    let rows: [(Keystroke, FilterEdit); 17] = [
        (
            typed(Modifiers::none(), "a", Some("a")),
            FilterEdit::Set("qa".into()),
        ),
        (typed(shift, "a", Some("A")), FilterEdit::Set("qA".into())),
        (
            typed(Modifiers::none(), "space", Some(" ")),
            FilterEdit::Set("q ".into()),
        ),
        (alt("e", Some("é")), FilterEdit::Set("qé".into())),
        (alt("e", None), FilterEdit::Set("q".into())),
        (key("a"), FilterEdit::Set("q".into())),
        (key("space"), FilterEdit::Set("q".into())),
        (key("backspace"), FilterEdit::Set(String::new())),
        (key("enter"), FilterEdit::Leave),
        (key("escape"), FilterEdit::Clear),
        (key("cmd-b"), FilterEdit::App),
        (
            typed(command, "enter", Some("\n")),
            FilterEdit::Set("q".into()),
        ),
        (key("ctrl-c"), FilterEdit::Terminal),
        (key("left"), FilterEdit::Terminal),
        (key("tab"), FilterEdit::Terminal),
        (key("fn-f5"), FilterEdit::Terminal),
        (alt("left", None), FilterEdit::Terminal),
    ];
    for (keystroke, expected) in rows {
        assert_eq!(
            filter_edit("q", &keystroke),
            expected,
            "{keystroke:?} in a field holding \"q\""
        );
    }
}

/// Focuses the field (holding "z") and sends one hardware key press.
fn press_in_field(
    keystroke: Keystroke,
    cx: &mut TestAppContext,
) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    let (view, receipts, cx) = terminal(cx);
    type_into_field(&view, &receipts, cx);
    assert!(
        field_focused(&view, cx),
        "fixture: the field holds the keys"
    );
    cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    cx.run_until_parked();
    (view, receipts, cx)
}

/// Asserts that `keystroke` gave the keys back and reached the terminal as
/// one key event whose debug text contains `wire`.
fn reaches_the_terminal(keystroke: &'static str, wire: &str, cx: &mut TestAppContext) {
    let (view, receipts, cx) = press_in_field(key(keystroke), cx);
    assert!(
        !field_focused(&view, cx),
        "{keystroke} left the keys with the field"
    );
    let events = sent(&view, &receipts, cx);
    assert!(
        events.len() == 1 && events[0].contains(wire),
        "{keystroke} did not reach the terminal once as {wire}: {events:?}"
    );
    assert_eq!(query(&view, cx), "z", "{keystroke} changed the query");
    assert!(
        view.read_with(cx, |this, _| this.shell.fit.hardware_keyboard),
        "{keystroke} forgot the hardware keyboard"
    );
}

#[gpui::test]
fn control_c_reaches_the_terminal(cx: &mut TestAppContext) {
    reaches_the_terminal("ctrl-c", "code: Char('c'), modifiers: 2,", cx);
}

#[gpui::test]
fn an_arrow_reaches_the_terminal(cx: &mut TestAppContext) {
    reaches_the_terminal("up", "code: Up,", cx);
}

#[gpui::test]
fn tab_reaches_the_terminal(cx: &mut TestAppContext) {
    reaches_the_terminal("tab", "code: Tab,", cx);
}

#[gpui::test]
fn a_function_key_reaches_the_terminal(cx: &mut TestAppContext) {
    reaches_the_terminal("f5", "code: F(5),", cx);
}

/// Option-composed text is the field's: it goes into the query, the field
/// keeps the keys and the terminal gets nothing (Opus SF1: before, Option-E
/// handed the keys over and "é" reached the remote shell).
#[gpui::test]
fn option_composed_text_stays_in_the_field(cx: &mut TestAppContext) {
    let (view, receipts, cx) = press_in_field(alt("e", Some("é")), cx);
    assert_eq!(query(&view, cx), "zé");
    assert!(
        field_focused(&view, cx),
        "Option-E took the keys from the field"
    );
    assert_eq!(sent(&view, &receipts, cx), Vec::<String>::new());
    assert!(
        view.read_with(cx, |this, _| this.shell.fit.hardware_keyboard),
        "Option-E forgot the hardware keyboard"
    );
}

/// An Option dead key (no text yet) is dropped in the field: nothing reaches
/// the terminal and the field keeps the keys.
#[gpui::test]
fn an_option_dead_key_is_dropped_in_the_field(cx: &mut TestAppContext) {
    let (view, receipts, cx) = press_in_field(alt("e", None), cx);
    assert_eq!(query(&view, cx), "z");
    assert!(field_focused(&view, cx));
    assert_eq!(sent(&view, &receipts, cx), Vec::<String>::new());
}

/// Return leaves the field with the query kept; the next key is the terminal's.
#[gpui::test]
fn return_leaves_the_field(cx: &mut TestAppContext) {
    let (view, receipts, cx) = press_in_field(key("enter"), cx);
    assert!(!field_focused(&view, cx));
    assert_eq!(query(&view, cx), "z");
    assert_eq!(
        sent(&view, &receipts, cx),
        Vec::<String>::new(),
        "Return reached the terminal"
    );
}

/// A Command shortcut is not the field's: it does not edit the query and is
/// not stopped there, so it goes on to the app's actions.
#[gpui::test]
fn a_command_shortcut_goes_on_to_the_app(cx: &mut TestAppContext) {
    let (view, receipts, cx) = press_in_field(key("cmd-k"), cx);
    assert_eq!(query(&view, cx), "z");
    assert!(field_focused(&view, cx));
    assert_eq!(sent(&view, &receipts, cx), Vec::<String>::new());
}

/// Command-Return carries "\n" (herdr-uikit named_key_char): the field drops
/// it, so it never reaches the terminal's text input and runs the shell line.
#[gpui::test]
fn command_return_does_not_reach_the_shell(cx: &mut TestAppContext) {
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    let (view, receipts, cx) = press_in_field(typed(command, "enter", Some("\n")), cx);
    assert_eq!(query(&view, cx), "z");
    assert!(field_focused(&view, cx));
    assert_eq!(sent(&view, &receipts, cx), Vec::<String>::new());
}
