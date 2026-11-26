//! A Find request that meets the reader mailbox busy is kept, not dropped (#160).
//!
//! The reader thread locks the mailbox while it applies each publication
//! (link.rs), and `open_find` only `try_lock`s it on the UI thread. A Find
//! pressed at that moment used to return without a bar; host tests saw it as
//! `resize_fits_the_painted_terminal_across_chrome_states` losing its Find
//! after a resize reply. Here a helper thread holds the mailbox at the exact
//! attempt, then lets go before the effects of that update run.
#![allow(clippy::unwrap_used)]
use super::regression_tests::install;
use crate::app::Smoke;
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{
    sync::mpsc::{Sender, channel},
    thread::{JoinHandle, spawn},
    time::Duration,
};

fn connected(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    // A quiet peer publishes nothing after startup, so only the helper below
    // ever holds the mailbox.
    let (client, _receipts) = crate::find::peer::connect_quiet(false).unwrap();
    install(&view, cx, "busy-open", client);
    (view, cx)
}

/// Locks the mailbox on another thread until the returned sender fires.
fn hold_mailbox(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> (Sender<()>, JoinHandle<()>) {
    let inbox = view.read_with(cx, |this, _| this.terminal.link.inbox.clone());
    let (locked, is_locked) = channel();
    let (release, released) = channel::<()>();
    let holder = spawn(move || {
        let guard = inbox.lock();
        let _ = locked.send(guard.is_ok());
        let _ = released.recv_timeout(Duration::from_secs(10));
    });
    assert_eq!(
        is_locked.recv_timeout(Duration::from_secs(10)).ok(),
        Some(true),
        "fixture: the helper never held the mailbox"
    );
    (release, holder)
}

#[gpui::test]
fn find_opens_once_the_busy_mailbox_is_released(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    let (release, holder) = hold_mailbox(&view, cx);
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.open_find(window, cx);
            assert!(this.find.is_none(), "fixture: the mailbox was free");
        });
        // Effects (and so the next draw) run when this update returns.
        let _ = release.send(());
    });
    holder.join().unwrap();
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |this, _| this.find.is_some()),
        "Find was dropped because the reader held the mailbox"
    );
}

#[gpui::test]
fn closing_withdraws_a_find_still_waiting_for_the_mailbox(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    let (release, holder) = hold_mailbox(&view, cx);
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.open_find(window, cx);
            this.close_find(window, cx);
        });
        let _ = release.send(());
    });
    holder.join().unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |this, _| this.find.is_none()));
    // A free mailbox opens Find at once: the withdrawal, not the fixture, kept it shut.
    cx.update(|window, app| view.update(app, |this, cx| this.open_find(window, cx)));
    assert!(view.read_with(cx, |this, _| this.find.is_some()));
}

#[gpui::test]
fn a_waiting_find_never_opens_for_another_board_selection(cx: &mut TestAppContext) {
    let (view, cx) = connected(cx);
    let (release, holder) = hold_mailbox(&view, cx);
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.open_find(window, cx);
            // The owner picked another row before the mailbox came free.
            this.board.selection = this.board.selection.wrapping_add(1);
        });
        let _ = release.send(());
    });
    holder.join().unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |this, _| this.find.is_none()));
}
