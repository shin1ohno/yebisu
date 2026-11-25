//! A background return reopens the Herdr 2 space that was shown, not the
//! space confirmed before it (`default` when none was). The shown space is
//! only confirmed when the view moves away from it while connected
//! (`remember_connected_core`), and Background drops the session first.
use crate::app::Smoke;
use gpui::{AppLifecyclePhase, Entity, TestAppContext};
use std::time::{Duration, Instant};

const HOST: &str = "private-core";

#[track_caller]
fn wait_connected(view: &Entity<Smoke>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.connected
    }) {
        assert!(
            Instant::now() < deadline,
            "Herdr 2 space did not connect: {}",
            view.update(cx, |this, _| this.terminal.live.status.clone())
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn backend() -> sheprd_core_client::CoreBackend {
    crate::core_mock::recorded(false)
        .unwrap_or_else(|e| panic!("peer: {e}"))
        .0
}

fn shown_space(view: &Entity<Smoke>, cx: &mut TestAppContext) -> Option<String> {
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .map(|core| core.name().to_owned())
    })
}

/// Opens `space-a` the way the sidebar or New space does, then leaves the app.
fn open_and_leave(view: &Entity<Smoke>, cx: &mut TestAppContext, automatic: bool) {
    view.update(cx, |this, cx| {
        // A saved host: the launch record only keeps those.
        this.setup.hosts.push(HOST.into());
        this.select_host(HOST);
        this.terminal.core_test_backend = Some(backend());
        this.connect_named_core(HOST.into(), "space-a".into(), None, cx)
            .unwrap_or_else(|e| panic!("open space-a: {e}"));
    });
    wait_connected(view, cx);
    assert_eq!(shown_space(view, cx).as_deref(), Some("space-a"));
    view.update(cx, |this, cx| {
        // Set once connected: connecting to a host may replace the shown view.
        this.terminal.reconnect.automatic = automatic;
        this.terminal.core_test_backend = Some(backend());
        this.lifecycle(AppLifecyclePhase::Inactive, cx);
        this.lifecycle(AppLifecyclePhase::Background, cx);
        assert!(this.terminal.core.is_none(), "Background ends the link");
    });
}

#[gpui::test]
fn automatic_return_reopens_the_shown_space(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    open_and_leave(&view, cx, true);
    view.update(cx, |this, cx| this.lifecycle(AppLifecyclePhase::Active, cx));
    assert_eq!(
        shown_space(&view, cx).as_deref(),
        Some("space-a"),
        "the return dialed another space"
    );
    wait_connected(&view, cx);
}

#[gpui::test]
fn reconnect_after_a_tap_return_reopens_the_shown_space(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    open_and_leave(&view, cx, false);
    view.update(cx, |this, cx| {
        this.lifecycle(AppLifecyclePhase::Active, cx);
        assert!(this.terminal.core.is_none(), "Tap reconnect waits");
        // What a cold launch would restore while it waits.
        let saved = this
            .launch_snapshot()
            .selected
            .and_then(|entry| entry.space);
        assert_eq!(saved.as_deref(), Some("space-a"));
        this.connect_terminal(cx);
    });
    assert_eq!(
        shown_space(&view, cx).as_deref(),
        Some("space-a"),
        "Reconnect dialed another space"
    );
    wait_connected(&view, cx);
}
