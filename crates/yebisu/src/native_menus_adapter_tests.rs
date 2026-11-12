//! Real consumer callbacks over a controlled peer, not UIKit/physical evidence.
use super::*;
use crate::mock_peer;
use gpui::{AppContext, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext};
use sheprd_client::protocol::ClientMessage;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn key(chord: &str, held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(chord).unwrap_or_else(|e| panic!("key {e}")),
        is_held: held,
        prefer_character_input: false,
    }
}
fn captured(this: &Smoke, command: Command) -> NativeCommand {
    let entry = actions::catalog(&this.terminal)
        .into_iter()
        .find(|a| a.info.command == command)
        .unwrap_or_else(|| panic!("catalog entry"));
    NativeCommand {
        name: entry.info.name.into(),
        revision: Some(this.native_menus.revision),
    }
}
fn local_state(this: &Smoke, command: Command) -> bool {
    match command {
        Command::ToggleSidebar => this.actions.board_hidden,
        Command::Palette => this.actions.palette,
        _ => panic!("not local"),
    }
}

#[gpui::test]
fn pending_rpc_local_callbacks_once_without_extra_wire(cx: &mut TestAppContext) {
    // Empty root prevents incidental rendering/resizing from masquerading as wire
    // caused by the callbacks under test. Install is still the real menu adapter.
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    let (client, receipts, release) =
        mock_peer::connect_controlled().unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "peer startup timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(window.is_window_active(), "fixture scene must be active");
            this.terminal_focus.focus(window, cx);
            this.install_native_menus(window, cx);
            this.terminal_key_down(&key("cmd-d", false), window, cx);
            this.terminal_key_up(
                &KeyUpEvent {
                    keystroke: key("cmd-d", false).keystroke,
                },
                window,
                cx,
            );
            assert!(
                this.terminal.navigating.is_some(),
                "initial split must be pending"
            );
        })
    });
    let received = receipts
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("initial receipt {e}"));
    let ClientMessage::ClientShellEndpointRequest { request, .. } = received else {
        panic!("expected initial split request, not input")
    };
    let request: serde_json::Value =
        serde_json::from_str(&request).unwrap_or_else(|e| panic!("request {e}"));
    assert_eq!(request["method"], "pane.split");

    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            for (command, chord) in [
                (Command::ToggleSidebar, "cmd-b"),
                (Command::Palette, "cmd-shift-p"),
            ] {
                this.install_native_menus(window, cx);
                let entries = actions::catalog(&this.terminal);
                let local = entries
                    .iter()
                    .find(|a| a.info.command == command)
                    .unwrap_or_else(|| panic!("local entry"));
                assert!(
                    local.available,
                    "pending local catalog entry must remain available"
                );
                assert!(
                    this.native_menus
                        .rows
                        .iter()
                        .any(|(name, _, enabled)| name == local.info.name && *enabled),
                    "D1 pending local menu must remain enabled"
                );
                let before = local_state(this, command);
                this.terminal_key_down(&key(chord, false), window, cx);
                assert_ne!(
                    local_state(this, command),
                    before,
                    "D1 pending local shortcut must toggle once"
                );
                this.terminal_key_down(&key(chord, true), window, cx);
                assert_ne!(
                    local_state(this, command),
                    before,
                    "held key must not toggle twice"
                );
                this.terminal_key_up(
                    &KeyUpEvent {
                        keystroke: key(chord, false).keystroke,
                    },
                    window,
                    cx,
                );
                // Pointer menu route, with replay of the exact captured revision.
                this.install_native_menus(window, cx);
                let action = captured(this, command);
                this.native_menu_action(&action, window, cx);
                assert_eq!(
                    local_state(this, command),
                    before,
                    "native pointer toggles once"
                );
                this.native_menu_action(&action, window, cx);
                assert_eq!(
                    local_state(this, command),
                    before,
                    "native replay must not toggle"
                );
                // Toolbar and palette use this exact consumer entry point.
                assert!(this.run_action(command).is_ok());
                assert_ne!(local_state(this, command), before, "toolbar local parity");
                assert!(this.run_action(command).is_ok());
                assert_eq!(local_state(this, command), before);
                assert!(
                    this.terminal.navigating.is_some(),
                    "local action must preserve pending RPC"
                );
            }
            this.install_native_menus(window, cx);
            assert!(
                !actions::catalog(&this.terminal)
                    .iter()
                    .any(|a| a.info.command == Command::SplitRight && a.available)
            );
            let remote = captured(this, Command::SplitRight);
            this.native_menu_action(&remote, window, cx);
            this.terminal_key_down(&key("cmd-d", false), window, cx);
            this.terminal_key_up(
                &KeyUpEvent {
                    keystroke: key("cmd-d", false).keystroke,
                },
                window,
                cx,
            );
            assert!(
                this.run_action(Command::SplitRight).is_err(),
                "pending remote must reject"
            );

            // Keep context rejection on the actual callback, including local actions.
            for mutation in 0..8 {
                this.install_native_menus(window, cx);
                let action = captured(this, Command::ToggleSidebar);
                let before = this.actions.board_hidden;
                let live = this.terminal.live.clone();
                let host = this.terminal.reconnect.target.clone();
                let generation = this.terminal.link.generation;
                match mutation {
                    0 => this.terminal.reconnect.target = "other-host".into(),
                    1 => this.terminal.link.generation += 1,
                    2 => {
                        let s = Arc::make_mut(
                            this.terminal
                                .live
                                .snapshot
                                .as_mut()
                                .unwrap_or_else(|| panic!("snapshot")),
                        );
                        s.boot_id = "other-boot".into();
                    }
                    3 => {
                        let s = Arc::make_mut(
                            this.terminal
                                .live
                                .snapshot
                                .as_mut()
                                .unwrap_or_else(|| panic!("snapshot")),
                        );
                        s.focused_pane_id = Some("other-pane".into());
                    }
                    4 => this.terminal.live.connected = false,
                    5 => this.focus.focus(window, cx),
                    6 => this.native_menus.revision += 1,
                    _ => this.terminal.input.composition.text = "かな".into(),
                }
                this.native_menu_action(&action, window, cx);
                assert_eq!(
                    this.actions.board_hidden, before,
                    "stale local callback {mutation}"
                );
                this.terminal.live = live;
                this.terminal.reconnect.target = host;
                this.terminal.link.generation = generation;
                this.terminal.input.composition.text.clear();
                this.terminal_focus.focus(window, cx);
            }
        })
    });
    // The server cannot read extra requests while its split gate is blocked.
    // Release it BEFORE checking the negative receipt, otherwise zero wire is vacuous.
    release.send(()).unwrap_or_else(|e| panic!("release {e}"));
    assert!(
        matches!(
            receipts.recv_timeout(Duration::from_millis(250)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ),
        "no additional RPC or terminal input"
    );
}
