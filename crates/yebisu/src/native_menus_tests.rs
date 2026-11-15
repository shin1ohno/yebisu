use super::*;
use crate::{live::Live, mock_peer};
use gpui::{AppContext, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext};
use sheprd_client::{
    ClientEvent, VersionMismatch,
    protocol::{ClientMessage, ClientPaneInputEvent, ClientShellPopupSurface},
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

/// The catalog rows the OS menu lists (all but search), aligned with its rows.
fn menu_catalog(terminal: &TerminalView, menus: &NativeMenus) -> Vec<actions::Action> {
    let rows: Vec<_> = actions::catalog(terminal)
        .into_iter()
        .filter(|row| row.info.command != Command::Find)
        .collect();
    assert_eq!(
        rows.len(),
        menus.rows.len(),
        "OS menu rows and catalog differ"
    );
    rows
}

/// First default shortcut of `command` on this host OS (`cmd-d` on macOS,
/// `ctrl-shift-d` on Linux). Expectations come from the catalog, never a literal.
fn default_shortcut(command: Command) -> &'static str {
    actions::catalog(&terminal())
        .into_iter()
        .find(|row| row.info.command == command)
        .and_then(|row| row.shortcuts.first().copied())
        .unwrap_or_else(|| panic!("no default shortcut for {command:?}"))
}

fn shortcut_key(command: Command) -> Keystroke {
    Keystroke::parse(default_shortcut(command)).unwrap_or_else(|e| panic!("key {e}"))
}

/// Menu glyphs for a shortcut, written independently of `menu_label`.
fn glyphs(shortcut: &str) -> String {
    shortcut
        .split("-")
        .map(|part| match part {
            "cmd" => "⌘".to_owned(),
            "ctrl" => "⌃".to_owned(),
            "alt" => "⌥".to_owned(),
            "shift" => "⇧".to_owned(),
            key => key.to_uppercase(),
        })
        .collect()
}

fn available(terminal: &TerminalView, actions: &actions::Actions, command: Command) -> bool {
    catalog_rows(terminal, actions, true)
        .iter()
        .any(|row| row.info.command == command && row.available)
}

// The Settings page is not mounted yet (P11), so this compares the menu rows
// with the shared catalog API only; it is not evidence about Settings.
#[test]
fn menu_rows_match_catalog_labels_shortcuts_and_availability() {
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
                    assert!(!row.shortcuts.is_empty());
                    assert!(label.contains(&glyphs(default_shortcut(Command::SplitRight))));
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
        if mutation == 3 {
            let snapshot = Arc::make_mut(
                t.live
                    .snapshot
                    .as_mut()
                    .unwrap_or_else(|| panic!("snapshot")),
            );
            let mut other = snapshot.panes[0].clone();
            other.pane_id = "w1:p3".into();
            other.focused = false;
            snapshot.panes.push(other);
        }
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
                let snapshot = Arc::make_mut(
                    t.live
                        .snapshot
                        .as_mut()
                        .unwrap_or_else(|| panic!("snapshot")),
                );
                snapshot.focused_pane_id = Some("w1:p3".into());
                for pane in &mut snapshot.panes {
                    pane.focused = Some(&pane.pane_id) == snapshot.focused_pane_id.as_ref();
                }
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
                    .is_ok(),
                "current key intent: mutation {mutation}"
            );
            assert!(menus.refresh(&t, &a, focused));
            assert!(
                menus
                    .command_gate(Command::SplitRight, true, &t, &a, focused)
                    .is_ok(),
                "refreshed menu: mutation {mutation}"
            );
        }
    }
}

#[test]
fn recognition_survives_capability_loss_but_preserves_policy_exclusions() {
    let mut t = terminal();
    let mut a = actions::Actions::default();
    let key = shortcut_key(Command::SplitRight);
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
        keystroke: shortcut_key(Command::SplitRight),
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
                    assert!(this.actions.consumed.contains(&key(false).keystroke.key));
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

#[test]
fn terminal_priority_with_command_control_disables_menu_rows_and_rejects_selection() {
    let mut t = terminal();
    let mut a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    for (priority, control) in [(false, false), (true, false), (false, true), (true, true)] {
        a.terminal_priority = priority;
        t.input.options.command_control = control;
        let masked = priority && control;
        menus.refresh(&t, &a, true);
        // Positive control: the same command is available when only one half is set.
        assert_eq!(
            available(&t, &a, Command::SplitRight),
            !masked,
            "priority={priority} command_control={control}"
        );
        for (row, (_, _, enabled)) in menu_catalog(&t, &menus).iter().zip(&menus.rows) {
            assert_eq!(*enabled, row.available && !masked, "row {}", row.info.name);
        }
        for installed in [false, true] {
            assert_eq!(
                menus
                    .command_gate(Command::SplitRight, installed, &t, &a, true)
                    .is_err(),
                masked,
                "installed={installed} priority={priority} command_control={control}"
            );
        }
    }
}

#[test]
fn shortcut_candidate_refuses_popup_and_closed_terminal() {
    let mut t = terminal();
    let a = actions::Actions::default();
    let key = shortcut_key(Command::SplitRight);
    assert_eq!(
        actions::shortcut_candidate(&t, &a, &key),
        Some(Command::SplitRight)
    );
    t.open = false;
    assert_eq!(actions::shortcut_candidate(&t, &a, &key), None, "closed");
    t.open = true;
    let surface = Arc::make_mut(t.live.surface.as_mut().unwrap_or_else(|| panic!("surface")));
    surface.popup = Some(Box::new(ClientShellPopupSurface {
        terminal_id: "popup".into(),
        title: "test".into(),
        width: None,
        height: None,
        frame: surface.frame.clone(),
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        pixel_width: 0,
        pixel_height: 0,
    }));
    assert_eq!(actions::shortcut_candidate(&t, &a, &key), None, "popup");
}

fn mismatch() -> VersionMismatch {
    VersionMismatch::DaemonOutdated {
        server_version: None,
    }
}

#[test]
fn version_mismatch_disables_menu_rows_and_rejects_selection_and_key() {
    let t = terminal();
    let a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    menus.refresh(&t, &a, true);
    assert!(available(&t, &a, Command::SplitRight), "positive control");
    assert!(
        menus
            .command_gate(Command::SplitRight, true, &t, &a, true)
            .is_ok()
    );
    let _ = t.live.version_mismatch.set(mismatch());
    assert!(!available(&t, &a, Command::SplitRight));
    menus.refresh(&t, &a, true);
    for (row, (_, _, enabled)) in menu_catalog(&t, &menus).iter().zip(&menus.rows) {
        if !matches!(row.info.command, Command::ToggleSidebar | Command::Palette) {
            assert!(!*enabled, "remote row {} stays enabled", row.info.name);
        }
    }
    for installed in [false, true] {
        assert!(
            menus
                .command_gate(Command::SplitRight, installed, &t, &a, true)
                .is_err()
        );
    }
}

#[gpui::test]
fn inbox_version_mismatch_refuses_run_action_without_wire(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|_, cx| {
        view.update(cx, |this, _| {
            this.terminal = terminal();
            assert!(available(
                &this.terminal,
                &this.actions,
                Command::SplitRight
            ));
            {
                // The shown state is healthy; only the inbox has seen the mismatch.
                let mut inbox = this
                    .terminal
                    .link
                    .inbox
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                inbox.connected = true;
                let _ = inbox.version_mismatch.set(mismatch());
            }
            let error = this
                .run_action(Command::SplitRight)
                .err()
                .unwrap_or_else(|| panic!("run_action must refuse"));
            assert!(
                error.to_string().contains("Host unavailable"),
                "refused for the wrong reason: {error}"
            );
        })
    });
}

#[gpui::test]
fn native_menus_register_no_key_bindings(cx: &mut TestAppContext) {
    fn native_binding(keymap: &gpui::Keymap) -> bool {
        keymap
            .bindings()
            .any(|binding| binding.action().as_any().is::<NativeCommand>())
    }
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal = terminal();
            this.terminal_focus.focus(window, cx);
            this.install_native_menus(window, cx);
            assert!(!this.native_menus.rows.is_empty(), "menus were not built");
        });
        // A displayed shortcut must stay text: a KeyBinding would make UIKit
        // build a UIKeyCommand and turn a menu pick into a key_down.
        assert!(!native_binding(&cx.key_bindings().borrow()));
        // Positive control: the predicate does see such a binding.
        let bound = gpui::Keymap::new(vec![gpui::KeyBinding::new(
            "cmd-d",
            NativeCommand {
                name: "split_right".into(),
            },
            None,
        )]);
        assert!(native_binding(&bound));
    });
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
            this.native_command_action(&command, window, cx);
            assert_ne!(this.actions.board_hidden, before, "first native selection");
            this.native_command_action(&command, window, cx);
            assert_eq!(this.actions.board_hidden, before, "second native selection");
            this.native_command_action(
                &NativeCommand {
                    name: "new_window".into(),
                },
                window,
                cx,
            );
            assert!(this.terminal.live.status.contains("unsupported command"));
            this.focus.focus(window, cx);
            this.native_command_action(&command, window, cx);
            assert_eq!(this.actions.board_hidden, before, "wrong focus must refuse");
        })
    });
}
