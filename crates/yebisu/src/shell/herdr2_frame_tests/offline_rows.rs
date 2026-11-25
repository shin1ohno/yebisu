//! A Herdr 2 host keeps its spaces in the sidebar when its link ends. The
//! owner saw every space, `default` included, leave the sidebar until they
//! made a new space (2026-11-26): the rows lived only as long as one
//! connected session. They now stay, marked not connected, and a tap opens
//! the space again.
use crate::{
    app::Smoke,
    core_mock::{Host, paste::Probe},
    shell::{
        herdr2_frame::sidebar::{CoreRow, herdr2_hosts},
        sidebar::model::HostLink,
    },
};
use gpui::{AppLifecyclePhase, Entity, TestAppContext};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const HOST: &str = "private-core";

#[track_caller]
fn wait(view: &Entity<Smoke>, cx: &mut TestAppContext, done: impl Fn(&Smoke) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        done(this)
    }) {
        assert!(
            Instant::now() < deadline,
            "not reached: {}",
            view.update(cx, |this, _| this.terminal.live.status.clone())
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The Herdr 2 rows the sidebar draws for `HOST`: (space id, detail).
fn rows(this: &Smoke) -> Option<(HostLink, Vec<(String, String)>)> {
    herdr2_hosts(&this.shell.sidebar, &this.terminal, &this.board.hidden_core)
        .into_iter()
        .find(|entry| entry.host == HOST)
        .map(|entry| {
            let spaces = entry
                .spaces
                .iter()
                .map(|space| (space.id.clone(), space.detail.clone()))
                .collect();
            (entry.link, spaces)
        })
}

fn listed(this: &Smoke) -> Vec<String> {
    rows(this)
        .map(|(_, spaces)| spaces.into_iter().map(|(id, _)| id).collect())
        .unwrap_or_default()
}

/// Connects `default` through a peer the test can hang up.
fn connect(view: &Entity<Smoke>, cx: &mut TestAppContext) -> Probe {
    let probe = Probe::default();
    let (backend, _) = Host::default()
        .with_paste(false, probe.clone())
        .unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.select_host(HOST);
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(view, cx, |this| {
        this.terminal.live.connected && listed(this).contains(&"default".to_owned())
    });
    probe
}

#[gpui::test]
fn a_dropped_link_keeps_the_hosts_spaces(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = connect(&view, cx);
    // The link ends under the session, as a lost SSH tunnel does.
    probe.close.store(true, Ordering::Release);
    wait(&view, cx, |this| !this.terminal.live.connected);
    view.update(cx, |this, _| {
        let (link, spaces) = rows(this).unwrap_or_else(|| panic!("the Herdr 2 host left"));
        assert_ne!(link, HostLink::Connected);
        let default = spaces
            .iter()
            .find(|(id, _)| id == "default")
            .unwrap_or_else(|| panic!("default left the sidebar: {spaces:?}"));
        assert_eq!(default.1, "Not connected");
    });
}

#[gpui::test]
fn a_space_without_a_session_stays_listed(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let _probe = connect(&view, cx);
    // Background ends the session (`reconnect.rs`); Detach does the same.
    view.update(cx, |this, cx| {
        this.terminal.reconnect.automatic = false;
        this.lifecycle(AppLifecyclePhase::Background, cx);
        this.lifecycle(AppLifecyclePhase::Active, cx);
        assert!(this.terminal.core.is_none());
        assert_eq!(listed(this), ["default"], "the paused host lost its spaces");
        this.detach_core();
        assert_eq!(
            listed(this),
            ["default"],
            "the detached host lost its spaces"
        );
    });
}

#[gpui::test]
fn tapping_a_listed_space_while_offline_opens_it(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = connect(&view, cx);
    probe.close.store(true, Ordering::Release);
    wait(&view, cx, |this| !this.terminal.live.connected);
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("second peer: {e}"));
    view.update(cx, |this, cx| {
        this.terminal.core_test_backend = Some(backend);
        let generation = this.terminal.link.generation;
        this.open_herdr2_row(HOST, "", generation, CoreRow::Space("default".into()), cx)
            .unwrap_or_else(|e| panic!("the offline row did not open: {e}"));
    });
    wait(&view, cx, |this| this.terminal.live.connected);
    view.update(cx, |this, _| {
        let name = this
            .terminal
            .core
            .as_ref()
            .map(|core| core.name().to_owned());
        assert_eq!(name.as_deref(), Some("default"));
        assert_eq!(rows(this).map(|(link, _)| link), Some(HostLink::Connected));
    });
}
