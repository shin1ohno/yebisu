//! Exercise the same lifecycle adapter registered by app::run with real wire peers.
use crate::{app::Smoke, mock_peer};
use gpui::{AppContext, AppLifecyclePhase as Phase, Entity, TestAppContext};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn ready(view: &Entity<Smoke>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.connected && this.terminal.live.surface.is_some()
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "peer did not publish a live frame"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn sh1_155_inactive_keeps_herdr1_identity_and_wire(cx: &mut TestAppContext) {
    for automatic in [true, false] {
        let view = cx.new(Smoke::new);
        let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, _| {
            this.select_host("private-peer");
            this.terminal.reconnect.automatic = automatic;
            let generation = this.terminal.link.reset();
            this.terminal.link.install(generation, Ok(client));
        });
        ready(&view, cx);
        view.update(cx, |this, cx| {
            let generation = this.terminal.link.generation;
            let inbox = this.terminal.link.inbox.clone();
            let reader = this.terminal.link.reader.as_ref().map(|r| r.thread().id());
            for phase in [
                Phase::Inactive,
                Phase::Foreground,
                Phase::Active,
                Phase::Active,
            ] {
                this.lifecycle(phase, cx);
                assert!(!this.terminal.reconnect.suspended);
                assert!(this.terminal.link.handle.is_some());
                assert_eq!(this.terminal.link.generation, generation);
                assert!(Arc::ptr_eq(&inbox, &this.terminal.link.inbox));
                assert_eq!(
                    this.terminal.link.reader.as_ref().map(|r| r.thread().id()),
                    reader
                );
                assert!(this.terminal.live.connected);
            }
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("handle"))
                .resize("boot-v1", sheprd_client::ConnectOptions::default())
                .unwrap_or_else(|e| panic!("wire: {e}"));
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let message = wire.recv_timeout(Duration::from_millis(100));
            if matches!(
                message,
                Ok(sheprd_client::protocol::ClientMessage::ClientShellResize { .. })
            ) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "same peer did not receive resize after return"
            );
        }
    }
}

#[gpui::test]
fn sh1_155_background_closes_herdr1_and_automatic_return_dials(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    let client = mock_peer::connect().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("private-peer");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    ready(&view, cx);
    let drained = view.update(cx, |this, cx| {
        let drained = this.terminal.link.drained.clone();
        this.lifecycle(Phase::Inactive, cx);
        this.lifecycle(Phase::Background, cx);
        assert!(this.terminal.reconnect.suspended);
        assert!(this.terminal.link.handle.is_none());
        assert!(!this.terminal.live.connected);
        drained
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !drained.load(std::sync::atomic::Ordering::Acquire) {
        assert!(Instant::now() < deadline, "Herdr 1 reader stayed connected");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        let generation = this.terminal.link.generation;
        this.lifecycle(Phase::Active, cx);
        assert!(
            this.terminal.link.generation > generation,
            "Automatic did not dial"
        );
        assert!(!this.terminal.reconnect.suspended);
    });
}

#[gpui::test]
fn sh1_155_inactive_keeps_hidden_herdr1(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    let client = mock_peer::connect().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        let hidden = this.board.hidden.entry("hidden".into()).or_default();
        hidden.reconnect.begin("hidden".into());
        let generation = hidden.link.reset();
        hidden.link.install(generation, Ok(client));
        let inbox = hidden.link.inbox.clone();
        this.lifecycle(Phase::Inactive, cx);
        this.lifecycle(Phase::Active, cx);
        let hidden = &this.board.hidden["hidden"];
        assert!(hidden.link.handle.is_some());
        assert!(Arc::ptr_eq(&inbox, &hidden.link.inbox));
        assert_eq!(hidden.link.generation, generation);
        this.lifecycle(Phase::Background, cx);
        assert!(this.board.hidden["hidden"].link.handle.is_none());
    });
}
