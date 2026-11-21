//! P13 device D1: no part of the Escape press that closes search reaches
//! terminal input, through repeat and release, in both keyboard-report modes,
//! and every new Escape press after it does.
//!
//! On device the retirement acknowledgement reopens terminal input while the
//! closing key is still down, and the replacement native view can deliver
//! that same press as a non-held KeyDown (find/closing_press.rs). The host
//! sequences below replay those shapes through the production key callbacks
//! and read the actual receiving socket. `TEST_OLDEST_HOLD` stands in for the
//! SDK press ledger, which needs UIKit. Each test runs both report modes
//! before asserting, so a failure shows both.
#![allow(clippy::unwrap_used)]
use super::regression_tests::{barrier, install, key, open};
use crate::{app::Smoke, find::closing_press::TEST_OLDEST_HOLD};
use gpui::{Entity, KeyUpEvent, TestAppContext, VisualTestContext};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::{sync::mpsc::Receiver, time::Duration};

/// The ledger holds a press that began before the close: the closing one.
const CLOSING_HELD: Option<Duration> = Some(Duration::from_secs(3600));
/// The ledger holds only a press that began just now.
const NEW_PRESS: Option<Duration> = Some(Duration::ZERO);

fn ledger(oldest: Option<Duration>) {
    TEST_OLDEST_HOLD.with(|hold| hold.set(oldest));
}

fn release(name: &'static str) -> KeyUpEvent {
    KeyUpEvent {
        keystroke: key(name, false).keystroke,
    }
}

/// Opens search on a fresh window, closes it with a non-held Escape, and
/// reopens terminal input through the correlated retirement boundary.
fn closed_by_escape<'a>(
    cx: &'a mut TestAppContext,
    host: &str,
    report_all: bool,
) -> (
    Entity<Smoke>,
    &'a mut VisualTestContext,
    Receiver<ClientMessage>,
) {
    ledger(CLOSING_HELD);
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect_quiet(report_all).unwrap();
    install(&view, cx, host, client);
    let field = open(&view, cx);
    cx.update(|window, cx| {
        field.update(cx, |field, cx| field.key(&key("escape", false), window, cx))
    });
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(this.find.is_none(), "D1 Escape did not close search");
            this.controlled_find_retirement(window);
            assert!(!this.find_handoff.paused, "D1 retirement kept the fence");
            // Release generated input without waiting for a surface resize.
            this.terminal.size = None;
        })
    });
    (view, cx, receipts)
}

/// Each key callback runs in its own update, so a draw can sync the surface
/// size in between; clearing it keeps generated input from waiting on a
/// resize that never comes on the host.
fn down(view: &Entity<Smoke>, cx: &mut VisualTestContext, held: bool) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.terminal_key_down(&key("escape", held), window, cx)
        })
    });
}

fn up(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.terminal_key_up(&release("escape"), window, cx)
        })
    });
}

/// The rest of the closing press after terminal input reopened, with the
/// ledger still holding it: timer repeats, the same press re-delivered as a
/// non-held KeyDown, more repeats, and its release unless `lost`.
fn rest_of_closing_press(view: &Entity<Smoke>, cx: &mut VisualTestContext, lost: bool) {
    ledger(CLOSING_HELD);
    down(view, cx, true);
    down(view, cx, true);
    down(view, cx, false);
    down(view, cx, true);
    if !lost {
        up(view, cx);
    }
}

/// One new Escape press with the ledger reading `oldest` at its KeyDown.
fn new_press(view: &Entity<Smoke>, cx: &mut VisualTestContext, oldest: Option<Duration>) {
    ledger(oldest);
    down(view, cx, false);
    up(view, cx);
}

fn kinds(events: &[ClientPaneInputEvent]) -> Vec<ClientKeyKind> {
    events
        .iter()
        .map(|event| match event {
            ClientPaneInputEvent::Key {
                code: ClientKeyCode::Esc,
                modifiers: 0,
                kind,
                ..
            } => *kind,
            other => panic!("D1 unexpected terminal input {other:?}"),
        })
        .collect()
}

/// What one new press must send: its Press, and its Release when every key
/// is reported.
fn once(report_all: bool) -> Vec<ClientKeyKind> {
    if report_all {
        vec![ClientKeyKind::Press, ClientKeyKind::Release]
    } else {
        vec![ClientKeyKind::Press]
    }
}

#[gpui::test]
fn p13_device_d1_closing_escape_press_never_reaches_terminal_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    let mut seen = Vec::new();
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-closing", report_all);
        rest_of_closing_press(&view, cx, false);
        let events = kinds(&barrier(&view, cx, &receipts, "d1-closing"));
        let latched = view.update(cx, |this, _| this.find_handoff.closing_press.is_some());
        seen.push((report_all, events, latched));
    }
    assert!(
        seen.iter().all(|(_, events, _)| events.is_empty()),
        "D1 closing Escape press reached terminal input: {seen:?}"
    );
    assert!(
        seen.iter().all(|(_, _, latched)| !latched),
        "D1 closing press outlived its release: {seen:?}"
    );
}

#[gpui::test]
fn p13_device_d1_fresh_escape_after_closing_release_reaches_terminal_once_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    let mut seen = Vec::new();
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-fresh", report_all);
        rest_of_closing_press(&view, cx, false);
        new_press(&view, cx, NEW_PRESS);
        let events = kinds(&barrier(&view, cx, &receipts, "d1-fresh"));
        seen.push((report_all, events == once(report_all), events));
    }
    assert!(
        seen.iter().all(|(_, exact, _)| *exact),
        "D1 fresh Escape after the closing release did not reach the terminal exactly once: {seen:?}"
    );
}

/// Review S1: a ledger read that shows nothing held (a cancel in flight, or
/// no window to read during a resize) while the press goes on must not end
/// the latch; the press stays with the UI until its own release.
#[gpui::test]
fn p13_device_d1_empty_ledger_read_mid_press_keeps_closing_press_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    let mut seen = Vec::new();
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-empty-ledger", report_all);
        ledger(None);
        down(&view, cx, true);
        down(&view, cx, true);
        rest_of_closing_press(&view, cx, false);
        let events = kinds(&barrier(&view, cx, &receipts, "d1-empty-ledger"));
        seen.push((report_all, events));
    }
    assert!(
        seen.iter().all(|(_, events)| events.is_empty()),
        "D1 S1 empty ledger read released the closing press: {seen:?}"
    );
}

/// Review S2: the closing press's release never reached GPUI, but the ledger
/// saw it (a retired view records releases it no longer dispatches). The
/// next Escape is a new press and reaches the terminal exactly once.
#[gpui::test]
fn p13_device_d1_new_escape_after_lost_closing_release_reaches_terminal_once_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    let mut seen = Vec::new();
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-lost-release", report_all);
        rest_of_closing_press(&view, cx, true);
        new_press(&view, cx, NEW_PRESS);
        let events = kinds(&barrier(&view, cx, &receipts, "d1-lost-release"));
        seen.push((report_all, events == once(report_all), events));
    }
    assert!(
        seen.iter().all(|(_, exact, _)| *exact),
        "D1 S2 new Escape after a lost closing release was not delivered exactly once: {seen:?}"
    );
}

/// The window lost activation while the closing press was down and neither
/// GPUI nor the ledger saw its release. The next Escape is a new press.
#[gpui::test]
fn p13_device_d1_new_escape_after_focus_loss_reaches_terminal_once_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    let mut seen = Vec::new();
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-focus-loss", report_all);
        // Activation observers fire on a change, so the window starts active.
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        rest_of_closing_press(&view, cx, true);
        cx.deactivate_window();
        cx.run_until_parked();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        // The stale entry keeps its age: a second Began of a held usage is
        // ignored by the ledger.
        new_press(&view, cx, CLOSING_HELD);
        let events = kinds(&barrier(&view, cx, &receipts, "d1-focus-loss"));
        seen.push((report_all, events == once(report_all), events));
    }
    assert!(
        seen.iter().all(|(_, exact, _)| *exact),
        "D1 new Escape after focus loss was not delivered exactly once: {seen:?}"
    );
}
