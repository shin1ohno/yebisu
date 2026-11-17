//! P13 device D1: no part of the Escape press that closes search reaches
//! terminal input, through repeat and release, in both keyboard-report modes.
//!
//! On device the retirement acknowledgement reopens terminal input while the
//! closing key is still down, and the replacement native view can deliver
//! that same press as a non-held KeyDown (find/closing_press.rs). The host
//! sequence below replays that shape through the production key callbacks
//! and reads the actual receiving socket.
#![allow(clippy::unwrap_used)]
use super::regression_tests::{barrier, install, key, open};
use crate::app::Smoke;
use gpui::{Entity, KeyUpEvent, TestAppContext, VisualTestContext};
use sheprd_client::protocol::{ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::sync::mpsc::Receiver;

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
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, host, client);
    view.update(cx, |this, _| {
        this.terminal.link.inbox.lock().unwrap().keyboard_report_all = report_all;
    });
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

/// The rest of the closing press after terminal input reopened: timer
/// repeats, the same press re-delivered as a non-held KeyDown, more
/// repeats, then its release.
fn rest_of_closing_press(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_key_down(&key("escape", true), window, cx);
            this.terminal_key_down(&key("escape", true), window, cx);
            this.terminal_key_down(&key("escape", false), window, cx);
            this.terminal_key_down(&key("escape", true), window, cx);
            this.terminal_key_up(&release("escape"), window, cx);
        })
    });
}

fn kinds(events: &[ClientPaneInputEvent]) -> Vec<ClientKeyKind> {
    events
        .iter()
        .map(|event| match event {
            ClientPaneInputEvent::Key { kind, .. } => *kind,
            other => panic!("D1 unexpected terminal input {other:?}"),
        })
        .collect()
}

#[gpui::test]
fn p13_device_d1_closing_escape_press_never_reaches_terminal_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-closing", report_all);
        rest_of_closing_press(&view, cx);
        let events = barrier(&view, cx, &receipts, "d1-closing");
        assert!(
            events.is_empty(),
            "D1 closing Escape press reached terminal input (report_all={report_all}): {events:?}"
        );
        assert!(
            !view.update(cx, |this, _| this.closing_press_owns("escape")),
            "D1 closing press outlived its release"
        );
    }
}

#[gpui::test]
fn p13_device_d1_fresh_escape_after_closing_release_reaches_terminal_once_in_both_report_modes(
    cx: &mut TestAppContext,
) {
    for report_all in [false, true] {
        let (view, cx, receipts) = closed_by_escape(cx, "d1-fresh", report_all);
        rest_of_closing_press(&view, cx);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_key_down(&key("escape", false), window, cx);
                this.terminal_key_up(&release("escape"), window, cx);
            })
        });
        let events = barrier(&view, cx, &receipts, "d1-fresh");
        let expected = if report_all {
            vec![ClientKeyKind::Press, ClientKeyKind::Release]
        } else {
            vec![ClientKeyKind::Press]
        };
        assert_eq!(
            kinds(&events),
            expected,
            "D1 fresh Escape after the closing release did not reach the terminal exactly once (report_all={report_all})"
        );
    }
}
