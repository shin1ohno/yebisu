use super::*;
use crate::{live::Live, mock_peer};
use gpui::{AppContext, TestAppContext};
use sheprd_client::{ClientEvent, ConnectOptions, protocol::ClientMessage};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn terminal() -> TerminalView {
    let mut live = Live::default();
    live.connected = true;
    live.methods = vec!["tab.create".into(), "tab.focus".into(), "pane.split".into()];
    live.apply(ClientEvent::Snapshot(Arc::new(
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}")),
    )));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    TerminalView {
        live,
        open: true,
        ..Default::default()
    }
}
#[test]
fn catalog_uses_upstream_names_defaults_and_availability() {
    let t = terminal();
    for action in catalog(&t) {
        assert!(!action.label.starts_with("commands-"));
        assert!(!action.label.contains('$'));
        assert_eq!(action.shortcuts, action.info.default_shortcuts());
    }
    assert_eq!(
        shortcut(
            &t,
            &Actions::default(),
            &Keystroke::parse("cmd-t").unwrap_or_else(|e| panic!("key {e}"))
        ),
        Some(Command::Tab)
    );
    assert!(
        catalog(&t)
            .iter()
            .any(|a| a.info.command == Command::TabNumber(1) && a.available)
    );
    assert!(
        catalog(&t)
            .iter()
            .any(|a| a.info.command == Command::TabNumber(2) && !a.available)
    );
    assert!(
        !catalog(&t)
            .iter()
            .any(|a| matches!(a.info.command, Command::ConvertToChat | Command::NewWindow))
    );
}
#[test]
fn shortcuts_leave_terminal_prefix_extra_modifiers_and_composition_alone() {
    let mut t = terminal();
    let mut state = Actions::default();
    for chord in [
        "ctrl-b",
        "ctrl-d",
        "ctrl-t",
        "ctrl-c",
        "alt-d",
        "ctrl-cmd-t",
        "cmd-alt-t",
        "cmd-shift-t",
        "cmd-9",
        "x",
    ] {
        assert_eq!(
            shortcut(
                &t,
                &state,
                &Keystroke::parse(chord).unwrap_or_else(|e| panic!("key {e}"))
            ),
            None,
            "{chord}"
        );
    }
    let key = Keystroke::parse("cmd-d").unwrap_or_else(|e| panic!("key {e}"));
    assert_eq!(shortcut(&t, &state, &key), Some(Command::SplitRight));
    t.input.options.command_control = true;
    state.terminal_priority = true;
    assert_eq!(shortcut(&t, &state, &key), None);
    state.terminal_priority = false;
    assert_eq!(shortcut(&t, &state, &key), Some(Command::SplitRight));
    t.input.composition.text = "かな".into();
    assert_eq!(shortcut(&t, &state, &key), None);
    t.input.composition.text.clear();
    t.open = false;
    assert_eq!(shortcut(&t, &state, &key), None);
}
fn ready(view: &gpui::Entity<Smoke>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.terminal.navigating.is_none()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "peer timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[gpui::test]
fn tab_and_pane_commands_fence_focus_resize_and_input_before_api_reply(cx: &mut TestAppContext) {
    for (command, method) in [
        (Command::Tab, "tab.create"),
        (Command::SplitRight, "pane.split"),
    ] {
        let view = cx.new(Smoke::new);
        let (client, receipts, release) =
            mock_peer::connect_controlled().unwrap_or_else(|e| panic!("peer {e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let g = this.terminal.link.reset();
            this.terminal.link.install(g, Ok(client));
        });
        ready(&view, cx);
        view.update(cx, |this, _| assert!(this.run_action(command).is_ok()));
        let message = receipts
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("wire {e}"));
        let ClientMessage::ClientShellEndpointRequest { request, .. } = message else {
            panic!("not API")
        };
        let value: serde_json::Value =
            serde_json::from_str(&request).unwrap_or_else(|e| panic!("json {e}"));
        assert_eq!(value["method"], method);
        assert_eq!(value["params"]["focus"], true);
        view.update(cx, |this, _| {
            let t = &mut this.terminal;
            let mut changed = snapshot(&t.live)
                .cloned()
                .unwrap_or_else(|| panic!("snapshot"));
            changed.revision += 1;
            // A newer coherent snapshot is not the command's response.
            t.live.apply(ClientEvent::Snapshot(Arc::new(changed)));
            crate::board::finish_navigation(t);
            assert!(t.navigating.is_some());
            t.resize_if_ready(ConnectOptions::default());
            assert!(t.size.is_none());
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            assert!(this.terminal.input.commit("must not replay").is_none());
            this.terminal.input.mark("旧かな", None);
            assert!(this.terminal.input.composition.text.is_empty());
            this.flush_terminal();
            assert!(this.run_action(Command::Tab).is_err());
            let g = this.terminal.link.generation;
            assert!(this.focus_tab("sandbox", "w1:t1", "boot-v1", g).is_err());
            assert!(
                this.navigate_board("sandbox", Target::Agent("w1:p1".into()), "boot-v1", g)
                    .is_err()
            );
        });
        assert!(
            receipts.recv_timeout(Duration::from_millis(120)).is_err(),
            "focus/resize/input escaped barrier"
        );
        release.send(()).unwrap_or_else(|e| panic!("release {e}"));
        ready(&view, cx);
        view.update(cx, |this, _| {
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            assert!(this.terminal.input.next().is_none());
            assert!(this.terminal.input.commit("fresh").is_some());
            assert_ne!(
                snapshot(&this.terminal.live).and_then(|s| s.focused_pane_id.as_deref()),
                Some("w1:p1")
            );
            this.terminal.resize_if_ready(ConnectOptions::default());
            assert!(this.terminal.size.is_some());
        });
        assert!(matches!(
            receipts.recv_timeout(Duration::from_secs(2)),
            Ok(ClientMessage::ClientShellResize { .. })
        ));
    }
}
#[gpui::test]
fn stale_tab_and_unimplemented_action_send_nothing(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let g = this.terminal.link.reset();
        this.terminal.link.install(g, Ok(client));
    });
    ready(&view, cx);
    view.update(cx, |this, _| {
        let g = this.terminal.link.generation;
        assert!(this.focus_tab("old-host", "w1:t1", "boot-v1", g).is_err());
        assert!(this.focus_tab("sandbox", "w1:t1", "old-boot", g).is_err());
        assert!(
            this.focus_tab("sandbox", "w1:t1", "boot-v1", g - 1)
                .is_err()
        );
        assert!(this.focus_tab("sandbox", "missing", "boot-v1", g).is_err());
        assert!(this.run_action(Command::ConvertToChat).is_err());
        assert!(this.run_action(Command::TabNumber(9)).is_err());
    });
    assert!(receipts.recv_timeout(Duration::from_millis(100)).is_err());
}

#[gpui::test]
fn real_key_adapter_runs_once_and_releases_without_terminal_input(cx: &mut TestAppContext) {
    use gpui::{KeyDownEvent, KeyUpEvent};
    for (name, method) in [("t", "tab.create"), ("{", "tab.focus"), ("}", "tab.focus")] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let (client, receipts, release) =
            mock_peer::connect_controlled().unwrap_or_else(|e| panic!("peer {e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let g = this.terminal.link.reset();
            this.terminal.link.install(g, Ok(client));
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.surface.is_some()
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                let keystroke = Keystroke {
                    key: name.into(),
                    key_char: None,
                    modifiers: gpui::Modifiers {
                        platform: true,
                        ..Default::default()
                    },
                };
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                assert!(this.terminal.navigating.is_some());
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: true,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
                assert!(this.actions.consumed.is_empty());
            })
        });
        loop {
            match receipts.recv_timeout(Duration::from_secs(2)) {
                Ok(ClientMessage::ClientShellEndpointRequest { request, .. }) => {
                    let request: serde_json::Value =
                        serde_json::from_str(&request).unwrap_or_else(|e| panic!("request {e}"));
                    assert_eq!(request["method"], method);
                    if method == "tab.focus" {
                        assert_eq!(request["params"]["tab_id"], "w1:t1");
                    }
                    break;
                }
                // The real view laid out once before the keydown.
                Ok(ClientMessage::ClientShellResize { .. }) => {}
                other => panic!("unexpected wire event {other:?}"),
            }
        }
        assert!(receipts.recv_timeout(Duration::from_millis(100)).is_err());
        release.send(()).unwrap_or_else(|e| panic!("release {e}"));
    }
}
#[test]
fn command_barrier_requires_its_reply_and_rejection_is_visible() {
    for failure in [false, true] {
        let mut t = terminal();
        t.live.pending_request = Some("p7".into());
        t.navigating = Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Command,
            request: "p7".into(),
        });
        t.live.apply(ClientEvent::Response {
            request_id: "unrelated".into(),
            response: serde_json::json!({"result":{}}),
        });
        crate::board::finish_navigation(&mut t);
        assert!(t.navigating.is_some());
        t.live.apply(ClientEvent::Response {
            request_id: "p7".into(),
            response: if failure {
                serde_json::json!({"error":"denied"})
            } else {
                serde_json::json!({"result":{}})
            },
        });
        crate::board::finish_navigation(&mut t);
        assert!(t.navigating.is_none());
        if failure {
            assert!(t.live.status.contains("denied"));
        }
    }
}

// UIKit a573fb3 keymap.rs clears Shift for symbols, but keeps it for letters
// and named keys. Expected events below are platform constants, not parse().
#[test]
fn uikit_shifted_symbol_shortcuts_match_real_events() {
    use gpui::Modifiers;
    let mut t = terminal();
    t.live
        .methods
        .extend(["pane.focus", "pane.zoom", "pane.close", "tab.close"].map(String::from));
    for (name, shift, alt, command) in [
        ("}", false, false, Command::NextTab),
        ("{", false, false, Command::PreviousTab),
        ("]", false, true, Command::NextPane),
        ("[", false, true, Command::PreviousPane),
        ("d", true, false, Command::SplitDown),
        ("p", true, false, Command::Palette),
        ("enter", true, false, Command::Zoom),
        ("w", true, false, Command::CloseTab),
    ] {
        let key = Keystroke {
            key: name.into(),
            key_char: None,
            modifiers: Modifiers {
                platform: true,
                shift,
                alt,
                ..Default::default()
            },
        };
        assert_eq!(
            shortcut(&t, &Actions::default(), &key),
            Some(command),
            "{name}"
        );
        let mut extra = key.clone();
        extra.modifiers.control = true;
        assert_eq!(shortcut(&t, &Actions::default(), &extra), None);
        let mut missing_command = key;
        missing_command.modifiers.platform = false;
        assert_eq!(shortcut(&t, &Actions::default(), &missing_command), None);
    }
}

#[gpui::test]
fn palette_escape_press_repeats_and_release_send_nothing(cx: &mut TestAppContext) {
    palette_escape_sequence(cx, false);
}

#[gpui::test]
fn palette_escape_fresh_press_recovers_after_release_lost_on_hosts(cx: &mut TestAppContext) {
    palette_escape_sequence(cx, true);
}

fn palette_escape_sequence(cx: &mut TestAppContext, lose_release: bool) {
    use gpui::{KeyDownEvent, KeyUpEvent, Modifiers};
    for report_all in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let (client, receipts) =
            mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let g = this.terminal.link.reset();
            this.terminal.link.install(g, Ok(client));
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.surface.is_some()
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                this.terminal
                    .link
                    .inbox
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .keyboard_report_all = report_all;
                this.terminal.size = None;
                this.refresh_terminal_input();
                assert!(this.terminal.input.ready);
                this.actions.palette = true;
                let key = Keystroke {
                    key: "escape".into(),
                    key_char: None,
                    modifiers: Modifiers::default(),
                };
                for is_held in [false, true, true, true] {
                    this.terminal_key_down(
                        &KeyDownEvent {
                            keystroke: key.clone(),
                            is_held,
                            prefer_character_input: false,
                        },
                        window,
                        cx,
                    );
                    assert!(!this.actions.palette);
                    assert!(this.actions.consumed.contains("escape"));
                }
                if lose_release {
                    // Mirror Hosts: blur, detach, then show setup. The terminal
                    // element's key-up callback is absent on that screen.
                    window.blur(cx);
                    this.terminal.input.detach();
                    this.board.selection = this.board.selection.wrapping_add(1);
                    this.terminal.open = false;
                    this.smoke_mode = false;
                    assert!(!this.terminal_focus.is_focused(window));
                    assert!(this.actions.consumed.contains("escape"));
                    // Release happens here on setup, without terminal_key_up.
                    this.select_host("sandbox");
                    assert!(this.terminal.open);
                    this.terminal_focus.focus(window, cx);
                    this.refresh_terminal_input();
                    assert!(this.terminal.input.ready);
                } else {
                    this.terminal_key_up(
                        &KeyUpEvent {
                            keystroke: key.clone(),
                        },
                        window,
                        cx,
                    );
                    assert!(this.actions.consumed.is_empty());
                }
                assert!(this.terminal.input.next().is_none());
            })
        });
        // Layout may resize once; no semantic input may reach the wire.
        while let Ok(message) = receipts.recv_timeout(Duration::from_millis(120)) {
            assert!(
                matches!(message, ClientMessage::ClientShellResize { .. }),
                "Escape leaked: {message:?}"
            );
        }
        // A fresh press reaches the terminal even if setup swallowed its release.
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let key = Keystroke {
                    key: "escape".into(),
                    key_char: None,
                    modifiers: Modifiers::default(),
                };
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: key.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                this.terminal_key_up(&KeyUpEvent { keystroke: key }, window, cx);
            })
        });
        assert!(matches!(
            receipts.recv_timeout(Duration::from_secs(2)),
            Ok(ClientMessage::ClientShellPaneInput { .. })
        ));
    }
}
