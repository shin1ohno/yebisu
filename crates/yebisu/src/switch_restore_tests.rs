//! Mac hardware-keyboard input loss, fix 3: after a switch or a finished
//! navigation the terminal gets its focus, text responder request and input
//! destination back without a tap, as closing the Shortcuts popover does.
//! A switch detached the input and a chrome tap moved focus or dismissed the
//! keyboard; typing then went nowhere until the terminal was clicked. These
//! prove app routing only, not UIKit's first responder on a Mac.
use crate::{app::Smoke, board::Navigation, board::Target, mock_peer};
use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

fn connected(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let (client, _receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        assert!(
            this.terminal.restore_keys,
            "a host switch asks for a restore"
        );
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

fn keyboard_requests(recorded: &mpsc::Receiver<serde_json::Value>) -> Vec<String> {
    recorded
        .try_iter()
        .filter(|record| record["kind"] == "keyboard-request")
        .map(|record| record["reason"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// A tab, space or pane switch as Herdr 1 runs it: the request is pending,
/// input is detached, and the tap took the focus.
fn start_switch(this: &mut Smoke, window: &mut gpui::Window, cx: &mut gpui::Context<Smoke>) {
    this.terminal.restore_keys = false;
    this.terminal.navigating = Some(Navigation {
        boot: String::new(),
        revision: 0,
        target: Target::Command,
        request: "switch-1".into(),
    });
    this.terminal.input.detach();
    this.focus.focus(window, cx);
}

#[gpui::test]
fn a_finished_navigation_gives_the_terminal_its_keys_back(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    let (records, recorded) = mpsc::sync_channel(256);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.observation = crate::observation::Observation::injected(records);
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(this.accepts_text_input(window, cx), "positive control");
            start_switch(this, window, cx);
            assert!(!this.accepts_text_input(window, cx));
            // Still navigating: nothing is restored yet.
            this.restore_pending_terminal_keys(window, cx);
            assert!(!this.accepts_text_input(window, cx));
            // Herdr answers the request (a refusal ends it the same way).
            this.terminal.live.failed_request = Some("switch-1".into());
            crate::board::finish_navigation(&mut this.terminal);
            assert!(this.terminal.navigating.is_none());
            this.restore_pending_terminal_keys(window, cx);
            assert!(
                this.terminal_focus.is_focused(window),
                "the terminal has its focus back"
            );
            assert!(this.terminal.input.ready, "the destination is back");
            assert!(this.accepts_text_input(window, cx));
            assert!(!this.terminal.restore_keys, "restored once");
        })
    });
    assert_eq!(keyboard_requests(&recorded), ["switch-restore"]);
}

#[gpui::test]
fn a_restore_waits_while_the_palette_owns_the_keys(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // As in the other host tests: no painted size to match.
            this.terminal.size = None;
            start_switch(this, window, cx);
            this.terminal.live.failed_request = Some("switch-1".into());
            crate::board::finish_navigation(&mut this.terminal);
            this.actions.palette = true;
            this.restore_pending_terminal_keys(window, cx);
            assert!(!this.terminal_focus.is_focused(window));
            assert!(this.terminal.restore_keys, "still pending");
            this.actions.palette = false;
            this.restore_pending_terminal_keys(window, cx);
            assert!(this.terminal_focus.is_focused(window));
            assert!(this.accepts_text_input(window, cx));
        })
    });
}
