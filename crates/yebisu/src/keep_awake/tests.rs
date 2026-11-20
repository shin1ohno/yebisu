use super::*;
use gpui::{AppContext, TestAppContext};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// A guard that counts its release, standing in for herdr-uikit's.
fn counted(released: &Arc<AtomicUsize>) -> ActivityGuard {
    let released = released.clone();
    ActivityGuard::new(move || {
        released.fetch_add(1, Ordering::SeqCst);
    })
}

fn held(awake: &mut KeepAwake, attempt: u64, released: &Arc<AtomicUsize>) {
    assert_eq!(
        awake.acquired(attempt, Ok(counted(released))).ok(),
        Some(Acquired::Held)
    );
}

#[test]
fn no_connected_host_asks_for_nothing() {
    let mut awake = KeepAwake::default();
    assert_eq!(awake.step([]), Step::Keep);
    assert_eq!(awake.step([false, false]), Step::Keep);
}

#[test]
fn any_connected_host_holds_until_none_is() {
    let mut awake = KeepAwake::default();
    let released = Arc::default();
    assert_eq!(awake.step([false, true]), Step::Acquire(1));
    // Still asking: a second connection does not ask twice.
    assert_eq!(awake.step([true, true]), Step::Keep);
    held(&mut awake, 1, &released);
    assert_eq!(awake.step([true, false]), Step::Keep);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(awake.step([false, false]), Step::Release);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(awake.step([false]), Step::Keep);
}

#[test]
fn background_releases_while_connected_and_the_front_holds_again() {
    let mut awake = KeepAwake::default();
    let released = Arc::default();
    assert_eq!(awake.step([true]), Step::Acquire(1));
    held(&mut awake, 1, &released);
    awake.lifecycle(AppLifecyclePhase::Inactive);
    assert_eq!(awake.step([true]), Step::Keep);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    awake.lifecycle(AppLifecyclePhase::Background);
    assert_eq!(awake.step([true]), Step::Release);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    awake.lifecycle(AppLifecyclePhase::Foreground);
    assert_eq!(awake.step([true]), Step::Acquire(2));
    held(&mut awake, 2, &released);
    awake.lifecycle(AppLifecyclePhase::Disconnected);
    assert_eq!(awake.step([true]), Step::Release);
    assert_eq!(released.load(Ordering::SeqCst), 2);
    awake.lifecycle(AppLifecyclePhase::Active);
    assert_eq!(awake.step([true]), Step::Acquire(3));
}

#[test]
fn a_guard_answering_a_released_attempt_is_dropped() {
    let mut awake = KeepAwake::default();
    let released = Arc::default();
    assert_eq!(awake.step([true]), Step::Acquire(1));
    assert_eq!(awake.step([false]), Step::Release);
    assert_eq!(awake.step([true]), Step::Acquire(2));
    let late = awake.acquired(1, Ok(counted(&released)));
    assert_eq!(late.ok(), Some(Acquired::Stale));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    held(&mut awake, 2, &released);
    assert_eq!(awake.step([false]), Step::Release);
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn a_refused_hold_is_asked_again_only_when_wanted_afresh() {
    let mut awake = KeepAwake::default();
    assert_eq!(awake.step([true]), Step::Acquire(1));
    let refused = awake.acquired(1, Err(std::io::Error::other("not main thread").into()));
    assert!(refused.is_err());
    assert_eq!(awake.step([true]), Step::Keep);
    // Nothing is held, so there is nothing to release.
    assert_eq!(awake.step([false]), Step::Keep);
    assert_eq!(awake.step([true]), Step::Acquire(2));
}

#[gpui::test]
fn a_connected_host_keeps_the_display_awake_until_it_disconnects(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| this.sync_keep_awake(cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    view.update(cx, |this, cx| {
        this.board
            .hidden
            .entry("b@host".into())
            .or_default()
            .live
            .connected = true;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    view.update(cx, |this, cx| {
        for terminal in this.board.hidden.values_mut() {
            terminal.live.connected = false;
        }
        this.terminal.live.connected = false;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[gpui::test]
fn backgrounding_releases_the_display_and_returning_holds_it(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    // Not open and with no target, so Background leaves this status as it
    // is: only the lifecycle releases the hold.
    view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    view.update(cx, |this, cx| {
        this.lifecycle(AppLifecyclePhase::Inactive, cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    view.update(cx, |this, cx| {
        this.lifecycle(AppLifecyclePhase::Background, cx);
        assert!(this.terminal.live.connected);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    view.update(cx, |this, cx| this.lifecycle(AppLifecyclePhase::Active, cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
}

#[gpui::test]
fn a_refused_platform_hold_waits_for_the_next_connection(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    cx.set_idle_sleep_prevention_fails(true);
    view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    cx.set_idle_sleep_prevention_fails(false);
    view.update(cx, |this, cx| this.sync_keep_awake(cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    view.update(cx, |this, cx| {
        this.terminal.live.connected = false;
        this.sync_keep_awake(cx);
        this.terminal.live.connected = true;
        this.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
}
