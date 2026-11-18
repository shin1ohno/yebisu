//! The filter field on the real screen (layout follow-up W1, L4 review S4):
//! a tap on the header field and hardware keys narrow the sidebar list
//! through `filter_rows`; Escape clears it and the rows come back. Keys typed
//! into the field never reach the terminal.
use crate::{app::Smoke, mock_peer};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px, size};
use sheprd_client::protocol::ClientMessage;
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

type Receipts = Receiver<ClientMessage>;

/// A Herdr 1 terminal on the private peer (one agent, "reviewer", in the
/// space "repo"), drawn at 1200×1000 with the sidebar list open.
fn terminal(cx: &mut TestAppContext) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("filter-screen");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, receipts, cx)
}

fn tap(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"));
    cx.simulate_click(
        point(
            at.origin.x + at.size.width / 2.,
            at.origin.y + at.size.height / 2.,
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
}

/// Whether the list offers "Clear filter" (the query left no row).
fn empty_result(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("filter-clear").is_some()
}

fn query(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.sidebar_filter().to_owned())
}

/// Whether the terminal sent pane input to the peer since the last call.
fn terminal_sent_input(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
) -> bool {
    for _ in 0..10 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut sent = false;
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        sent |= matches!(message, ClientMessage::ClientShellPaneInput { .. });
    }
    sent
}

#[gpui::test]
fn typing_in_the_filter_field_narrows_the_list(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    assert!(!empty_result(cx), "fixture: the list has rows");
    terminal_sent_input(&view, &receipts, cx);

    // A query that matches no space and no agent.
    tap(cx, "sidebar-filter");
    cx.simulate_keystrokes("z q x");
    assert_eq!(query(&view, cx), "zqx");
    assert!(empty_result(cx), "a query matching nothing kept rows");
    assert!(
        !terminal_sent_input(&view, &receipts, cx),
        "a key typed into the filter field reached the terminal"
    );

    // Backspace edits the query; the agent's name matches again.
    cx.simulate_keystrokes("backspace backspace backspace r e v");
    assert_eq!(query(&view, cx), "rev");
    assert!(!empty_result(cx), "the agent reviewer was filtered out");

    // A query matching nothing, then Escape: cleared, rows back, and the
    // keys go to the terminal again.
    cx.simulate_keystrokes("z z");
    assert!(empty_result(cx));
    cx.simulate_keystrokes("escape");
    assert_eq!(query(&view, cx), "");
    assert!(!empty_result(cx), "clearing did not restore the rows");
    cx.simulate_keystrokes("a");
    assert!(
        terminal_sent_input(&view, &receipts, cx),
        "after Escape a key should reach the terminal again"
    );
}

#[gpui::test]
fn filter_field_clear_button_restores_the_list(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    tap(cx, "sidebar-filter");
    cx.simulate_keystrokes("n o p e");
    assert!(empty_result(cx));
    tap(cx, "sidebar-filter-clear");
    assert_eq!(query(&view, cx), "");
    assert!(!empty_result(cx));
}
