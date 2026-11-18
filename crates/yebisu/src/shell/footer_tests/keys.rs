use super::*;
use crate::shell::footer::keys::KeyOwner;
use sheprd_client::protocol::ClientPaneInputEvent;
use sheprd_commands::Command;

/// Pane input the peer received, after pushing out anything the terminal
/// still queues (the fixture peer keeps its size, so a resize would hold it).
fn pane_input(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &mpsc::Receiver<ClientMessage>,
) -> Vec<ClientPaneInputEvent> {
    view.update(cx, |this, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.flush_terminal();
    });
    cx.run_until_parked();
    drain(wire, Duration::from_millis(300))
        .into_iter()
        .flat_map(|message| match message {
            ClientMessage::ClientShellPaneInput { events, .. } => events,
            _ => Vec::new(),
        })
        .collect()
}

/// Review cf8bbcccc2164abe F1 (S5): while the Hosts sheet that footer Hosts
/// opened is on screen, hardware keys go to the sheet; neither the KeyDown
/// nor the typed text reaches the terminal or the remote shell. The keys go
/// through GPUI's real dispatch (`simulate_keystrokes`), so the test fails
/// when the footer leaves the window focus on the terminal.
#[gpui::test]
fn hosts_sheet_keys_never_reach_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    cx.update(|window, cx| view.update(cx, |this, cx| this.terminal_focus.focus(window, cx)));
    cx.run_until_parked();
    drain(&wire, Duration::from_millis(100));
    // Positive control: with the terminal focused, a letter reaches the shell.
    cx.simulate_keystrokes("a");
    let before = pane_input(&view, cx, &wire);
    assert!(
        !before.is_empty(),
        "positive control: the letter must reach the shell"
    );
    view.update(cx, |this, _| {
        assert_eq!(this.key_owner(), KeyOwner::Terminal)
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.footer_tap(FooterItem::Hosts, window, cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert_eq!(this.key_owner(), KeyOwner::HostsSheet);
            assert!(!this.terminal_owns_text_input());
            assert!(
                !this.terminal_focus.is_focused(window),
                "terminal kept focus"
            );
            assert!(this.focus.is_focused(window), "the sheet has no focus");
        })
    });
    cx.simulate_keystrokes("l s enter ctrl-c");
    cx.run_until_parked();
    let after = pane_input(&view, cx, &wire);
    assert!(
        after.is_empty(),
        "keys reached the remote shell while the Hosts sheet was open: {after:?}"
    );
    // Actions refuse while the sheet is open (A6 item 1, actions.rs).
    view.update(cx, |this, _| {
        assert!(this.input_modal());
        assert!(this.run_action(Command::Tab).is_err());
    });
    // A6 item 3: closing the sheet gives the focus and the keys back to the
    // terminal, without a tap, as closing Settings does.
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.hosts_close(cx);
            this.return_keys_to_terminal(window, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert_eq!(this.key_owner(), KeyOwner::Terminal);
            assert!(this.terminal_owns_text_input());
            assert!(!this.input_modal());
            assert!(this.terminal_focus.is_focused(window), "focus not returned");
        })
    });
    cx.simulate_keystrokes("z");
    let back = pane_input(&view, cx, &wire);
    assert!(!back.is_empty(), "keys did not return to the terminal");
}

/// Review 9714586129254e1d F1: the rail's Hosts (L4) is the same entry as
/// footer Hosts, so a press on it over a live terminal hands the keys to the
/// sheet too: the window focus moves to the sheet and typed keys never reach
/// the remote shell. The press goes through the painted rail button.
#[gpui::test]
fn rail_hosts_hands_keys_to_the_sheet(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    // The user chose the rail (L4 patch 02 draws it at any width).
    view.update(cx, |this, cx| {
        this.actions.board_hidden = true;
        cx.notify();
    });
    cx.update(|window, cx| view.update(cx, |this, cx| this.terminal_focus.focus(window, cx)));
    cx.run_until_parked();
    drain(&wire, Duration::from_millis(100));
    cx.simulate_keystrokes("a");
    assert!(
        !pane_input(&view, cx, &wire).is_empty(),
        "positive control: the letter must reach the shell"
    );
    let button = cx
        .debug_bounds("rail-hosts")
        .unwrap_or_else(|| panic!("the rail's Hosts is not painted"));
    cx.simulate_click(button.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(
                this.hosts_sheet_modal(),
                "the rail's Hosts did not open the sheet"
            );
            assert_eq!(this.key_owner(), KeyOwner::HostsSheet);
            assert!(
                !this.terminal_focus.is_focused(window),
                "terminal kept focus"
            );
            assert!(this.focus.is_focused(window), "the sheet has no focus");
        })
    });
    cx.simulate_keystrokes("l s enter");
    cx.run_until_parked();
    let after = pane_input(&view, cx, &wire);
    assert!(
        after.is_empty(),
        "keys reached the remote shell after the rail's Hosts: {after:?}"
    );
}

/// A6 item 2: opening the Hosts sheet over the terminal clears the
/// terminal's input state as opening Settings does: Select mode and the
/// handoff stamp end, and no pending focus request takes the keys back.
#[gpui::test]
fn hosts_sheet_open_clears_terminal_input_state(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.handoff.select_mode = true;
            this.terminal.needs_focus = true;
            this.sync_handoff();
            assert!(this.handoff.stamp.is_some(), "fixture has a stamp");
            this.footer_tap(FooterItem::Hosts, window, cx);
            assert!(
                !this.handoff.select_mode,
                "Select stayed on under the sheet"
            );
            assert!(this.handoff.stamp.is_none(), "handoff kept its stamp");
            assert!(!this.terminal.needs_focus, "a focus request is pending");
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(!this.terminal_focus.is_focused(window));
        })
    });
}

/// A6 item 1 (review ab783fbc2a944abc S2): every input guard patch 02 moves
/// off `settings_modal` reads this one helper, so one truth table covers
/// them: Settings or a Hosts sheet over a terminal owns the input; a Hosts
/// sheet without a terminal (the first-run frame) does not.
#[gpui::test]
fn input_modal_covers_settings_and_hosts_sheet(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        this.terminal.open = true;
        assert!(!this.input_modal(), "nothing open");
        this.settings_modal = true;
        assert!(this.input_modal(), "Settings");
        assert_eq!(this.key_owner(), KeyOwner::SettingsSheet);
        this.settings_modal = false;
        this.shell.hosts_sheet.open_list();
        assert!(this.input_modal(), "Hosts sheet over a terminal");
        assert!(!this.terminal_owns_text_input());
        this.terminal.open = false;
        assert!(!this.input_modal(), "first-run sheet with no terminal");
        this.shell.hosts_sheet.close();
        assert!(!this.input_modal());
    });
}
