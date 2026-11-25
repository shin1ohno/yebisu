//! The status bar reports a Herdr 2 session that is still dialing, or whose
//! dial failed, as what it is, not as "Connected" (owner Q4, 2026-11-25).
use super::*;
use crate::shell::overflow_menu::{
    OverflowEntry, OverflowItem, STILL_CONNECTING, overflow_menu_items,
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const HOST: &str = "private-core";

#[track_caller]
fn wait_until(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    predicate: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            predicate(this)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn reconnect_item(state: &ChromeState) -> OverflowEntry {
    overflow_menu_items(state)
        .into_iter()
        .find(|entry| entry.item == OverflowItem::Reconnect)
        .unwrap_or_else(|| panic!("Reconnect missing from the ... menu"))
}

fn dial_finished(this: &Smoke) -> bool {
    this.terminal
        .core
        .as_ref()
        .is_some_and(crate::core::Session::finished)
}

#[gpui::test]
fn herdr2_dial_shows_connecting_then_disconnected_with_reconnect(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (release, wait) = mpsc::channel::<()>();
    view.update(cx, |this, cx| {
        this.select_host(HOST);
        this.connect_core_using(cx, move |_| {
            wait.recv().map_err(|e| crate::Error::Ssh(e.to_string()))?;
            Err(crate::Error::Ssh("dial refused by test".into()))
        });
    });
    view.update(cx, |this, _| {
        let state = this.chrome_state();
        assert_eq!(state.kind, HerdrKind::Herdr2);
        assert!(this.terminal.core.is_some(), "the dial was not started");
        assert!(!state.connected, "a dial in progress is not a connection");
        assert!(state.connecting);
        let model = this.current_status_bar_model(&state);
        assert_eq!(model.state, "Connecting");
        assert!(!model.show_reconnect, "no Reconnect while the dial runs");
        let reconnect = reconnect_item(&state);
        assert!(!reconnect.enabled);
        assert_eq!(reconnect.reason, Some(STILL_CONNECTING));
    });
    release.send(()).unwrap_or_else(|e| panic!("release: {e}"));
    wait_until(&view, cx, "the failed dial to finish", dial_finished);
    view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        let state = this.chrome_state();
        assert!(!state.connected && !state.connecting, "{state:?}");
        let model = this.current_status_bar_model(&state);
        assert_eq!(model.state, "Disconnected");
        assert!(model.show_reconnect, "a failed dial must offer Reconnect");
        assert!(reconnect_item(&state).enabled);
        assert!(
            this.terminal.live.status.contains("dial refused by test"),
            "the failure reason is kept: {}",
            this.terminal.live.status
        );
    });
}

#[gpui::test]
fn herdr2_is_connected_only_after_its_first_frame(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (backend, _wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.select_host(HOST);
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_until(&view, cx, "the first Herdr 2 frame", |this| {
        this.terminal.live.connected
    });
    view.update(cx, |this, _| {
        let state = this.chrome_state();
        assert!(state.connected && !state.connecting, "{state:?}");
        let model = this.current_status_bar_model(&state);
        assert_eq!(model.state, "Connected");
        assert!(!model.show_reconnect);
        this.terminal.core.take();
    });
}

#[test]
fn connecting_status_line_names_the_state() {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connecting: true,
        ..ChromeState::default()
    };
    let model = status_bar_model(&state, "pro-dev");
    assert_eq!(status_line(&model), "pro-dev · Herdr 2 · Connecting");
    assert_eq!(model.accessibility_label, "pro-dev, Herdr 2, Connecting");
    assert!(!state.offers_reconnect());
}
