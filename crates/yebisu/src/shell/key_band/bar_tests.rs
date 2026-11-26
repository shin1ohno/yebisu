//! The keyboard bar's keys (#229): with a docked software keyboard there is
//! no in-app band, and the bar's presses (herdr-uikit's stream, here called
//! directly) latch and type as the band's taps do.
use super::tests::{float_keyboard, install, received, set_insets, wait_for_terminal};
use super::*;
use crate::mock_peer;
use gpui::{ClipboardItem, TestAppContext, VisualTestContext, size};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};

fn down(code: ClientKeyCode, modifiers: u8) -> ClientPaneInputEvent {
    ClientPaneInputEvent::Key {
        code,
        modifiers,
        kind: ClientKeyKind::Press,
        repeat_count: 1,
        shifted_codepoint: None,
        generated_text: None,
        tracks_release: false,
        physical_key_id: None,
        windows_record: None,
    }
}

fn index(name: &str) -> u32 {
    let index = keys::ALL
        .iter()
        .position(|key| key.name == name)
        .unwrap_or_else(|| panic!("no key {name}"));
    keys::id(index)
}

fn press(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext, name: &str) {
    let id = index(name);
    view.update_in(cx, |app, window, cx| app.press_bar_key(id, window, cx));
    cx.run_until_parked();
}

fn selected(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext) -> Vec<&'static str> {
    view.read_with(cx, |app, _| {
        app.keyboard_bar_model()
            .map(|model| {
                model
                    .leading
                    .iter()
                    .chain(&model.trailing)
                    .flatten()
                    .filter(|button| button.selected)
                    .map(|button| button.key.name)
                    .collect()
            })
            .unwrap_or_default()
    })
}

fn docked_terminal(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<Smoke>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<ClientMessage>,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    let (client, replies) =
        mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    wait_for_terminal(&view, cx);
    set_insets(&view, cx, 498., false);
    wait_for_terminal(&view, cx);
    cx.update(|window, app| {
        view.update(app, |view, cx| view.terminal_focus.focus(window, cx));
    });
    cx.run_until_parked();
    (view, cx, replies)
}

#[gpui::test]
fn the_bars_ctrl_latch_then_a_software_letter_sends_control_c(cx: &mut TestAppContext) {
    let (view, cx, replies) = docked_terminal(cx);
    assert!(cx.debug_bounds("shell-key-band").is_none(), "docked band");
    assert_eq!(
        view.update_in(cx, |app, window, _| app.key_surface_now(window)),
        bar::Surface::Bar
    );
    // Negative control: without the latch the letter is typed.
    cx.simulate_input("c");
    assert_eq!(
        received(&replies),
        ClientPaneInputEvent::TextCommit("c".into())
    );

    press(&view, cx, "ctrl");
    assert_eq!(selected(&view, cx), ["ctrl"]);
    cx.simulate_input("c");
    assert_eq!(received(&replies), down(ClientKeyCode::Char('c'), 2));
    // The latch is spent, and the bar no longer draws it.
    assert_eq!(selected(&view, cx), Vec::<&str>::new());
    cx.simulate_input("a");
    assert_eq!(
        received(&replies),
        ClientPaneInputEvent::TextCommit("a".into())
    );

    // Pressing a latch twice releases it.
    press(&view, cx, "ctrl");
    press(&view, cx, "ctrl");
    assert_eq!(selected(&view, cx), Vec::<&str>::new());
}

#[gpui::test]
fn the_bars_keys_and_menu_entries_press_the_bands_keys(cx: &mut TestAppContext) {
    let (view, cx, replies) = docked_terminal(cx);
    for (name, code) in [
        ("esc", ClientKeyCode::Esc),
        ("tab", ClientKeyCode::Tab),
        ("left", ClientKeyCode::Left),
        ("up", ClientKeyCode::Up),
        ("down", ClientKeyCode::Down),
        ("right", ClientKeyCode::Right),
    ] {
        press(&view, cx, name);
        assert_eq!(received(&replies), down(code, 0), "{name}");
    }
    for (name, text) in [
        ("backtick", "`"),
        ("caret", "^"),
        ("backslash", "\\"),
        ("bracket-left", "["),
        ("bracket-right", "]"),
        ("less", "<"),
        ("greater", ">"),
        ("pipe", "|"),
        ("tilde", "~"),
        ("slash", "/"),
        ("minus", "-"),
    ] {
        press(&view, cx, name);
        assert_eq!(
            received(&replies),
            ClientPaneInputEvent::TextCommit(text.into()),
            "{name}"
        );
    }
    press(&view, cx, "alt");
    press(&view, cx, "ctrl");
    assert_eq!(selected(&view, cx), ["ctrl", "alt"]);
    press(&view, cx, "bracket-left");
    assert_eq!(received(&replies), down(ClientKeyCode::Char('['), 6));

    cx.write_to_clipboard(ClipboardItem::new_string("keyboard bar paste".into()));
    press(&view, cx, "paste");
    assert_eq!(
        received(&replies),
        ClientPaneInputEvent::Paste("keyboard bar paste".into())
    );
}

#[gpui::test]
fn the_bars_command_latch_runs_the_shortcut_a_hardware_chord_runs(cx: &mut TestAppContext) {
    let (view, cx, replies) = docked_terminal(cx);
    // Negative control: a plain slash is typed and opens nothing.
    cx.simulate_input("/");
    assert_eq!(
        received(&replies),
        ClientPaneInputEvent::TextCommit("/".into())
    );
    assert!(!view.read_with(cx, |app, _| app.shell.shortcuts.open));
    // The hardware chord, for the expected effect.
    cx.simulate_keystrokes("cmd-/");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |app, _| app.shell.shortcuts.open),
        "hardware ⌘/"
    );
    view.update(cx, |app, cx| {
        app.shell.shortcuts.open = false;
        cx.notify();
    });
    cx.run_until_parked();

    press(&view, cx, "cmd");
    assert_eq!(selected(&view, cx), ["cmd"]);
    cx.simulate_input("/");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |app, _| app.shell.shortcuts.open),
        "bar ⌘ then /"
    );
    assert_eq!(selected(&view, cx), Vec::<&str>::new());
}

#[gpui::test]
fn the_latch_survives_on_the_bar_and_ends_with_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx, _replies) = docked_terminal(cx);
    press(&view, cx, "alt");
    assert!(view.read_with(cx, |app, _| app.shell.frame.key_band.is_armed()));
    // Floating keeps it (the band shows the same keys).
    float_keyboard(&view, cx);
    assert!(view.read_with(cx, |app, _| app.shell.frame.key_band.is_armed()));
    // A hardware keyboard alone shows no keys: the latch is spent.
    set_insets(&view, cx, 498., true);
    assert!(!view.read_with(cx, |app, _| app.shell.frame.key_band.is_armed()));
}
