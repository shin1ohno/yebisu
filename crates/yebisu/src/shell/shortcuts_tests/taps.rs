//! A tap on a popover row runs it: Find opens the find bar (review B1),
//! the local commands run locally, the RPC commands send their request; a
//! disabled row runs nothing.
use super::super::*;
use crate::{live::Live, mock_peer};
use gpui::{Entity, TestAppContext, VisualTestContext};
use sheprd_client::{ClientEvent, protocol::ClientShellSnapshot};
use sheprd_commands::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn connected(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
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
    (view, cx)
}

#[gpui::test]
fn tapping_each_enabled_row_runs_it(cx: &mut TestAppContext) {
    let enabled: Vec<Command> = {
        let (view, cx) = connected(cx);
        view.update(cx, |this, _| {
            let rows = popover_rows(&this.terminal, &this.popover_chrome_state());
            let enabled: Vec<Command> = rows
                .iter()
                .filter(|(_, row)| row.enabled)
                .map(|(command, _)| *command)
                .collect();
            for needed in [Command::Tab, Command::ToggleSidebar, Command::Palette] {
                assert!(enabled.contains(&needed), "{needed:?} live: {rows:?}");
            }
            enabled
        })
    };
    // One fresh connection per row: a sent command fences the next one.
    for command in enabled {
        let (view, cx) = connected(cx);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                let hidden = this.actions.board_hidden;
                assert!(this.open_shortcuts(cx));
                assert!(this.tap_shortcut_row(command, window, cx), "{command:?}");
                assert!(!this.shell.shortcuts.open, "{command:?} closes the popover");
                let status = this.terminal.live.status.clone();
                match command {
                    Command::Find => assert!(this.find.is_some(), "Find: {status}"),
                    Command::Palette => assert!(this.actions.palette),
                    Command::ToggleSidebar => assert_ne!(this.actions.board_hidden, hidden),
                    _ => assert!(
                        this.terminal.navigating.is_some(),
                        "{command:?} sent nothing: {status}"
                    ),
                }
            })
        });
    }
}

#[gpui::test]
fn disabled_row_tap_runs_nothing(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            assert!(this.open_shortcuts(cx));
            // Without the method the row is disabled.
            this.terminal.live.methods.retain(|m| m != "tab.create");
            assert!(!this.tap_shortcut_row(Command::Tab, window, cx));
            assert!(this.terminal.navigating.is_none());
            assert!(this.shell.shortcuts.open, "a disabled row keeps it open");
            // A closed popover runs nothing either.
            this.shell.shortcuts.open = false;
            assert!(!this.tap_shortcut_row(Command::Palette, window, cx));
            assert!(!this.actions.palette);
        })
    });
}

#[gpui::test]
fn find_row_opens_the_find_bar(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host("a");
        let mut live = Live::default();
        live.connected = true;
        live.methods = vec!["pane.copy_search".into()];
        let snapshot: ClientShellSnapshot =
            serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}"));
        live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
        live.apply(ClientEvent::Surface(Arc::new(crate::find::peer::surface())));
        this.terminal.live = live.clone();
        // `open_find` reads the reader's inbox too.
        *this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| panic!("inbox {e}")) = live;
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let find = popover_rows(&this.terminal, &this.popover_chrome_state())
                .into_iter()
                .find(|(command, _)| *command == Command::Find);
            assert!(find.is_some_and(|(_, row)| row.enabled), "Find is live");
            assert!(this.open_shortcuts(cx));
            assert!(this.tap_shortcut_row(Command::Find, window, cx));
            // The find bar opens; it is not sent as an RPC command (B1).
            assert!(this.find.is_some(), "{}", this.terminal.live.status);
            assert!(this.terminal.navigating.is_none());
            assert!(!this.shell.shortcuts.open);
        })
    });
}
