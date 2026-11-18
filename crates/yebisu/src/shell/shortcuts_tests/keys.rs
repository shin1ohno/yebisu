//! Keys never reach the terminal while the popover is open; Escape closes
//! it (review S1, integrator patch 05: `input.rs`, `terminal_controls.rs`).
use super::super::*;
use crate::mock_peer;
use gpui::{EntityInputHandler, KeyDownEvent, Keystroke, Modifiers, TestAppContext};
use std::time::{Duration, Instant};

fn key(name: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            key: name.into(),
            key_char: None,
            modifiers: Modifiers::default(),
        },
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
fn popover_holds_the_keyboard_and_escape_closes_it(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let (client, _receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let g = this.terminal.link.reset();
        this.terminal.link.install(g, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < deadline, "peer timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(this.terminal.input.ready, "positive control: input is live");
            assert!(this.accepts_text_input(window, cx));
            assert!(this.open_shortcuts(cx));
            // Text and keys stay out of the terminal while it is open.
            this.refresh_terminal_input();
            assert!(!this.terminal.input.ready);
            assert!(!this.accepts_text_input(window, cx));
            this.terminal_key_down(&key("a"), window, cx);
            assert!(this.shell.shortcuts.open, "a key does not close it");
            // Escape closes it and is consumed, not sent to the host.
            this.terminal_key_down(&key("escape"), window, cx);
            assert!(!this.shell.shortcuts.open);
            assert!(this.actions.consumed.contains("escape"));
            this.refresh_terminal_input();
            assert!(this.terminal.input.ready, "input is live again");
        })
    });
}

/// Review adf0e048f49b466e SF1: opening the popover clears Select mode, and
/// closing it by the Shortcuts toggle gives the terminal its focus and text
/// input back without a tap, as closing Settings does.
#[gpui::test]
fn closing_the_popover_returns_the_terminal_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let (client, _receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let g = this.terminal.link.reset();
        this.terminal.link.install(g, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < deadline, "peer timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(this.terminal.input.ready, "positive control: input is live");
            this.handoff.select_mode = true;
            assert!(this.toggle_shortcuts(window, cx));
            assert!(this.shell.shortcuts.open);
            assert!(!this.handoff.select_mode, "opening ends Select mode");
            // Something else took the focus while the popover was open.
            this.focus.focus(window, cx);
            assert!(this.toggle_shortcuts(window, cx));
            assert!(!this.shell.shortcuts.open);
            assert!(this.terminal.input.ready, "text input is live again");
            assert!(
                this.terminal_focus.is_focused(window),
                "closing must give the terminal its focus back"
            );
            assert!(this.accepts_text_input(window, cx));
        })
    });
}
