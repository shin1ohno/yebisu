use super::*;
use crate::{live::Live, mock_peer};
use gpui::{AppContext, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext};
use sheprd_client::{
    ClientEvent,
    protocol::{ClientMessage, ClientPaneInputEvent},
};
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
fn settings_and_menu_share_labels_shortcuts_and_availability() {
    let mut t = terminal();
    let a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    for focused in [true, false] {
        for connected in [true, false] {
            t.live.connected = connected;
            menus.refresh(&t, &a, focused);
            let settings = catalog_rows(&t, &a, focused);
            assert_eq!(settings.len(), menus.rows.len());
            for (row, (name, label, enabled)) in settings.iter().zip(&menus.rows) {
                assert_eq!(row.info.name, name);
                assert_eq!(menu_label(row), *label);
                assert_eq!(row.available, *enabled);
                assert_eq!(row.shortcuts, row.info.default_shortcuts());
                assert!(!matches!(name.as_str(), "copy" | "paste" | "new_window"));
                if row.info.command == Command::SplitRight {
                    assert!(label.contains("⌘D"));
                }
                if row.info.command == Command::ToggleSidebar {
                    assert_eq!(*enabled, focused);
                }
            }
        }
    }
}

#[test]
fn displayed_scope_changes_reject_but_physical_keys_use_current_state() {
    for mutation in 0..10 {
        let mut t = terminal();
        let a = actions::Actions::default();
        let mut menus = NativeMenus::default();
        menus.refresh(&t, &a, true);
        assert!(
            menus
                .command_gate(Command::SplitRight, true, &t, &a, true)
                .is_ok()
        );
        let mut focused = true;
        match mutation {
            0 => t.reconnect.target = "other".into(),
            1 => t.link.generation += 1,
            2 => {
                Arc::make_mut(
                    t.live
                        .snapshot
                        .as_mut()
                        .unwrap_or_else(|| panic!("snapshot")),
                )
                .boot_id = "other".into()
            }
            3 => {
                Arc::make_mut(
                    t.live
                        .snapshot
                        .as_mut()
                        .unwrap_or_else(|| panic!("snapshot")),
                )
                .focused_pane_id = Some("other".into())
            }
            4 => t.live.input_epoch += 1,
            5 => t.live.methods.clear(),
            6 => focused = false,
            7 => t.input.composition.text = "かな".into(),
            8 => t.live.connected = false,
            _ => t.reconnect.suspended = true,
        }
        assert!(
            menus
                .command_gate(Command::SplitRight, true, &t, &a, focused)
                .is_err(),
            "mutation {mutation}"
        );
        if mutation < 5 {
            assert!(
                menus
                    .command_gate(Command::SplitRight, false, &t, &a, focused)
                    .is_ok()
            );
        }
    }
}

#[test]
fn recognition_survives_capability_loss_but_preserves_policy_exclusions() {
    let mut t = terminal();
    let mut a = actions::Actions::default();
    let key = Keystroke::parse("cmd-d").unwrap_or_else(|e| panic!("key {e}"));
    t.live.methods.clear();
    for command_control in [false, true] {
        for priority in [false, true] {
            t.input.options.command_control = command_control;
            a.terminal_priority = priority;
            let excluded = command_control && priority;
            assert_eq!(
                actions::shortcut_candidate(&t, &a, &key),
                if excluded {
                    None
                } else {
                    Some(Command::SplitRight)
                }
            );
            assert!(
                NativeMenus::default()
                    .command_gate(Command::SplitRight, false, &t, &a, true)
                    .is_err()
            );
        }
    }
    a.terminal_priority = false;
    t.input.composition.text = "かな".into();
    assert_eq!(actions::shortcut_candidate(&t, &a, &key), None);
    t.input.composition.text.clear();
    t.core_mode = true;
    assert_eq!(actions::shortcut_candidate(&t, &a, &key), None);
}

fn key(held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse("cmd-d").unwrap_or_else(|e| panic!("key {e}")),
        is_held: held,
        prefer_character_input: false,
    }
}

#[gpui::test]
fn unavailable_shortcut_adapter_consumes_press_repeat_release_without_wire(
    cx: &mut TestAppContext,
) {
    for priority in [false, true] {
        let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let view = cx.new(Smoke::new);
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
                this.terminal.input.options.command_control = true;
                this.actions.terminal_priority = priority;
                this.terminal.live.methods.clear();
                this.terminal
                    .link
                    .inbox
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .methods
                    .clear();
                this.terminal_key_down(&key(false), window, cx);
                if !priority {
                    assert!(this.actions.consumed.contains("d"));
                }
                this.terminal_key_down(&key(true), window, cx);
                this.terminal_key_up(
                    &KeyUpEvent {
                        keystroke: key(false).keystroke,
                    },
                    window,
                    cx,
                );
                assert!(this.actions.consumed.is_empty());
                this.terminal
                    .link
                    .handle
                    .as_ref()
                    .unwrap_or_else(|| panic!("handle"))
                    .send_input(
                        "boot-v1",
                        "w1:p1",
                        [ClientPaneInputEvent::Paste("barrier".into())],
                    )
                    .unwrap_or_else(|e| panic!("fence {e}"));
            })
        });
        let mut inputs = Vec::new();
        loop {
            let message = receipts
                .recv_timeout(Duration::from_secs(3))
                .unwrap_or_else(|e| panic!("wire {e}"));
            match message {
                ClientMessage::ClientShellPaneInput { events, .. } => {
                    if events == [ClientPaneInputEvent::Paste("barrier".into())] {
                        break;
                    }
                    inputs.extend(events);
                }
                ClientMessage::ClientShellResize { .. } => {}
                other => panic!("unexpected wire {other:?}"),
            }
        }
        if priority {
            assert!(!inputs.is_empty(), "priority must preserve terminal input");
        } else {
            assert!(inputs.is_empty(), "unavailable recognized shortcut leaked");
        }
    }
}

#[gpui::test]
fn two_native_local_selections_each_execute_once_and_unknown_ids_refuse(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal = terminal();
            this.terminal.live.connected = false;
            this.terminal_focus.focus(window, cx);
            this.install_native_menus(window, cx);
            let name = catalog_rows(&this.terminal, &this.actions, true)
                .iter()
                .find(|row| row.info.command == Command::ToggleSidebar)
                .unwrap_or_else(|| panic!("sidebar"))
                .info
                .name
                .to_owned();
            let command = NativeCommand { name };
            let before = this.actions.board_hidden;
            this.native_menu_action(&command, window, cx);
            assert_ne!(this.actions.board_hidden, before, "first native selection");
            this.native_menu_action(&command, window, cx);
            assert_eq!(this.actions.board_hidden, before, "second native selection");
            this.native_menu_action(
                &NativeCommand {
                    name: "new_window".into(),
                },
                window,
                cx,
            );
            assert!(this.terminal.live.status.contains("unsupported command"));
            this.focus.focus(window, cx);
            this.native_menu_action(&command, window, cx);
            assert_eq!(this.actions.board_hidden, before, "wrong focus must refuse");
        })
    });
}
