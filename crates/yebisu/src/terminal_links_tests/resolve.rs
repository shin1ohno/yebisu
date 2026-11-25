//! An address at a row edge opens only as Herdr resolves it (review F1), and
//! a Command held before another app came to the front no longer counts
//! (review F2). The Herdr 1 wire peer answers `pane.link.resolve` as a
//! terminal that knows its wraps: the fixture's wrapped address spans its
//! rows, the address ending at the edge of row 18 does not.
use super::tap::{
    command, command_click_opens, connected, finger, press, received, settle_open, target,
};
use crate::app::Smoke;
use crate::terminal_links::{Refusal, qa};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};

/// A render, as a frame after an activation change would make.
fn redraw(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// The wrapped address opens whole once Herdr says it wraps, and the status
/// line shows it whole, since no row shows it so.
#[gpui::test]
fn a_wrapped_address_opens_whole_once_herdr_proves_the_wrap(cx: &mut TestAppContext) {
    command_click_opens(
        cx,
        "wrapped",
        qa::WRAPPED,
        &format!("Opening {}", qa::WRAPPED),
    );
}

/// F1: the address ending at the right edge, under a new line that starts
/// with `tail`, opens as shown, not joined with `tail`.
#[gpui::test]
fn an_address_ending_at_the_edge_opens_as_shown(cx: &mut TestAppContext) {
    command_click_opens(cx, "edge", qa::EDGE, "Opening example.com in the browser");
}

/// Without `pane.link.resolve` (a Herdr 1 daemon that lacks it; Herdr 2 is
/// `core_open.rs`), an address at a row edge opens nothing and the status
/// line says why.
#[gpui::test]
fn an_unproven_address_opens_nothing_where_herdr_cannot_resolve(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    view.update(cx, |this, _| {
        if let Ok(mut live) = this.terminal.link.inbox.lock() {
            live.methods.retain(|method| method != "pane.link.resolve");
        }
    });
    for name in ["wrapped", "edge"] {
        let at = target(&view, cx, name);
        press(cx, at, command());
        settle_open(&view, cx, 1);
        assert_eq!(cx.opened_url(), None, "{name}: opened without proof");
        view.update(cx, |this, _| {
            assert_eq!(this.terminal_status(), Refusal::Unproven.reason(), "{name}");
        });
    }
    assert_eq!(received(&view, cx, &wire), [], "a refused press sent input");
}

/// F2: Command held for one open, then released while Safari was in front:
/// the release never reached yebisu, so back in front a plain finger tap
/// must be a plain tap, until the keyboard reports Command again.
#[gpui::test]
fn a_command_held_before_another_app_came_to_the_front_no_longer_counts(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    let at = target(&view, cx, "plain");
    cx.simulate_modifiers_change(command());
    // Another app in front, then yebisu again; the window still holds the
    // Command it last heard of.
    cx.deactivate_window();
    redraw(&view, cx);
    cx.update(|window, _| window.activate_window());
    redraw(&view, cx);
    view.update(cx, |this, _| {
        assert!(!this.terminal.link_hover.trusts_keyboard(), "still trusted");
    });
    finger(cx, at);
    assert_eq!(cx.opened_url(), None, "a plain tap opened after Safari");
    view.update(cx, |this, _| {
        assert!(this.handoff.passive_highlight, "not today's tap");
    });
    // The keyboard speaks again: Command counts.
    cx.simulate_modifiers_change(Modifiers::default());
    cx.simulate_modifiers_change(command());
    finger(cx, at);
    assert_eq!(cx.opened_url().as_deref(), Some(qa::PLAIN));
}
