use super::*;
use crate::{live::Live, mock_peer};
use gpui::{AppContext, TestAppContext};
use sheprd_client::{ClientEvent, protocol::ClientMessage};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn terminal() -> TerminalView {
    let mut live = Live::default();
    live.connected = true;
    live.methods = vec!["tab.create".into(), "pane.split".into()];
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
fn action(menus: &NativeMenus, terminal: &TerminalView, command: Command) -> NativeCommand {
    NativeCommand {
        name: actions::catalog(terminal)
            .into_iter()
            .find(|a| a.info.command == command)
            .unwrap_or_else(|| panic!("catalog entry"))
            .info
            .name
            .into(),
        revision: Some(menus.revision),
    }
}

#[test]
fn catalog_rows_and_shortcuts_share_supported_commands() {
    let t = terminal();
    let a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    assert!(menus.refresh(&t, &a, true));
    assert!(!menus.refresh(&t, &a, true));
    let catalog = actions::catalog(&t);
    assert_eq!(menus.rows.len(), catalog.len());
    for (row, entry) in menus.rows.iter().zip(catalog) {
        assert_eq!(row, &(entry.info.name.into(), entry.label, entry.available));
        assert_eq!(entry.shortcuts, entry.info.default_shortcuts());
        assert!(!matches!(
            entry.info.command,
            Command::ConvertToChat | Command::NewWindow
        ));
    }
}

#[test]
fn stale_contexts_and_unsupported_ids_are_rejected() {
    for mutation in 0..13 {
        let mut t = terminal();
        let mut a = actions::Actions::default();
        let mut menus = NativeMenus::default();
        menus.refresh(&t, &a, true);
        let command = action(&menus, &t, Command::SplitRight);
        let mut focused = true;
        match mutation {
            0 => t.reconnect.target = "other-host".into(),
            1 => t.link.generation += 1,
            2 => t.live.connected = false,
            3 => t.live.methods.clear(),
            4 => t.reconnect.suspended = true,
            5 => t.input.composition.text = "かな".into(),
            6 => focused = false,
            7 => a.palette = true,
            8 => {
                a.terminal_priority = true;
                t.input.options.command_control = true;
            }
            9 => t.core_mode = true,
            10 => t.live.input_epoch += 1,
            11 => {
                let s = Arc::make_mut(
                    t.live
                        .snapshot
                        .as_mut()
                        .unwrap_or_else(|| panic!("snapshot")),
                );
                s.boot_id = "other-boot".into();
            }
            _ => {
                let s = Arc::make_mut(
                    t.live
                        .snapshot
                        .as_mut()
                        .unwrap_or_else(|| panic!("snapshot")),
                );
                s.focused_pane_id = Some("other-pane".into());
            }
        }
        assert!(
            menus.take(&command, &t, &a, focused).is_err(),
            "mutation {mutation}"
        );
    }
    let t = terminal();
    let a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    menus.refresh(&t, &a, true);
    for name in ["chat", "desktop", "copy", "paste", "not-a-command"] {
        let forged = NativeCommand {
            name: name.into(),
            revision: Some(menus.revision),
        };
        assert!(menus.take(&forged, &t, &a, true).is_err());
    }
}

#[test]
fn revision_consumed_once_and_display_only_binding_never_executes() {
    let t = terminal();
    let a = actions::Actions::default();
    let mut menus = NativeMenus::default();
    menus.refresh(&t, &a, true);
    let command = action(&menus, &t, Command::ToggleSidebar);
    let display = NativeCommand {
        name: command.name.clone(),
        revision: None,
    };
    assert_eq!(display, command); // Equality is ONLY shortcut lookup.
    assert!(menus.take(&display, &t, &a, true).is_err());
    assert_eq!(
        menus.take(&command, &t, &a, true).ok(),
        Some(Command::ToggleSidebar)
    );
    assert!(menus.take(&command, &t, &a, true).is_err());
    menus.refresh(&t, &a, true);
    assert!(menus.take(&command, &t, &a, true).is_err());
}

#[gpui::test]
fn consumer_dispatch_has_one_peer_request_and_rechecks_inbox_capability(cx: &mut TestAppContext) {
    for revoke in [false, true] {
        let view = cx.new(Smoke::new);
        let (client, receipts, _release) =
            mock_peer::connect_controlled().unwrap_or_else(|e| panic!("peer {e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let generation = this.terminal.link.reset();
            this.terminal.link.install(generation, Ok(client));
            this.terminal.open = true;
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.surface.is_some()
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "peer timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
        view.update(cx, |this, _| {
            this.native_menus
                .refresh(&this.terminal, &this.actions, true);
            if revoke {
                this.terminal
                    .link
                    .inbox
                    .lock()
                    .unwrap_or_else(|e| panic!("lock {e}"))
                    .methods
                    .clear();
            }
            let first = this.run_menu_shortcut(Command::SplitRight);
            assert_eq!(first.is_ok(), !revoke);
            assert!(this.run_menu_shortcut(Command::SplitRight).is_err());
        });
        if !revoke {
            let received = receipts
                .recv_timeout(Duration::from_secs(3))
                .unwrap_or_else(|e| panic!("receipt {e}"));
            let ClientMessage::ClientShellEndpointRequest { request, .. } = received else {
                panic!("expected API, not terminal input")
            };
            let json: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|e| panic!("request {e}"));
            assert_eq!(json["method"], "pane.split");
        }
        assert!(receipts.recv_timeout(Duration::from_millis(50)).is_err());
    }
}
