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
/// space "repo"), drawn at 1200×1000 with the sidebar list open, on an iPad
/// with a hardware keyboard.
fn terminal(cx: &mut TestAppContext) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    let (view, receipts, cx) = bare_terminal(cx);
    view.update(cx, |this, cx| {
        this.shell.fit.hardware_keyboard = true;
        cx.notify();
    });
    cx.run_until_parked();
    (view, receipts, cx)
}

/// As `terminal`, with no hardware keyboard known yet.
fn bare_terminal(cx: &mut TestAppContext) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
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

/// Focuses the field and types one character into it.
fn type_into_field(view: &Entity<Smoke>, receipts: &Receipts, cx: &mut VisualTestContext) {
    terminal_sent_input(view, receipts, cx);
    tap(cx, "sidebar-filter");
    cx.simulate_keystrokes("z");
    assert_eq!(query(view, cx), "z", "fixture: the field took the key");
    assert!(!terminal_sent_input(view, receipts, cx));
}

/// Review F1, scene A: the focused field leaves the screen when the sidebar
/// collapses to the rail (Command-B or Hide sidebar); the keys come back to
/// the terminal.
#[gpui::test]
fn keys_return_to_terminal_when_the_sidebar_collapses(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    type_into_field(&view, &receipts, cx);
    view.update(cx, |this, cx| {
        this.toggle_sidebar();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-filter").is_none(), "fixture: rail");
    cx.simulate_keystrokes("a");
    assert!(
        terminal_sent_input(&view, &receipts, cx),
        "a key after the sidebar collapsed reached no one"
    );
}

/// Review F1, scene A: the window narrows until the rail is forced.
#[gpui::test]
fn keys_return_to_terminal_when_the_rail_is_forced(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    type_into_field(&view, &receipts, cx);
    cx.simulate_resize(size(px(440.), px(1000.)));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-filter").is_none(), "fixture: rail");
    cx.simulate_keystrokes("a");
    assert!(
        terminal_sent_input(&view, &receipts, cx),
        "a key after the rail was forced reached no one"
    );
}

/// Review F1, scene B: with no hardware keyboard the field shows why it takes
/// no keys, and a tap on it leaves the keys with the terminal.
#[gpui::test]
fn field_takes_no_keys_without_a_hardware_keyboard(cx: &mut TestAppContext) {
    let (view, receipts, cx) = bare_terminal(cx);
    terminal_sent_input(&view, &receipts, cx);
    assert!(
        cx.debug_bounds("sidebar-filter-unavailable").is_some(),
        "the field does not say it needs a hardware keyboard"
    );
    assert!(cx.debug_bounds("sidebar-filter").is_none());
    tap(cx, "sidebar-filter-unavailable");
    cx.simulate_keystrokes("a");
    assert_eq!(query(&view, cx), "");
    assert!(
        terminal_sent_input(&view, &receipts, cx),
        "a tap on the field kept the keys from the terminal"
    );
}

/// Review F1, scene B with a query left from a hardware keyboard: the field
/// and its × stay, but a tap does not take the keys.
#[gpui::test]
fn set_query_field_takes_no_focus_without_a_hardware_keyboard(cx: &mut TestAppContext) {
    let (view, receipts, cx) = bare_terminal(cx);
    view.update(cx, |this, cx| {
        this.set_sidebar_filter("rev");
        cx.notify();
    });
    cx.run_until_parked();
    terminal_sent_input(&view, &receipts, cx);
    tap(cx, "sidebar-filter");
    cx.simulate_keystrokes("a");
    assert_eq!(query(&view, cx), "rev");
    assert!(
        terminal_sent_input(&view, &receipts, cx),
        "a tap on the field kept the keys from the terminal"
    );
}

/// Review F4: Herdr 2's sidebar does not filter, so the field takes no keys
/// there and says why.
#[gpui::test]
fn herdr2_field_takes_no_keys(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.update(cx, |this, cx| {
        this.terminal.core_mode = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-filter").is_none());
    let reason = view.update(cx, |this, cx| {
        this.render_filter_field(cx);
        this.shell.rail.filter_drawn
    });
    assert!(!reason, "the Herdr 2 field was drawn as a key target");
}
