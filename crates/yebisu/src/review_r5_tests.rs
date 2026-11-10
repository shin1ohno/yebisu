//! Deterministic schedules from the R3/R4 review, with receiving-peer evidence.
use crate::{
    app::Smoke,
    board::{self, Target},
    mock_peer,
};
use gpui::{AppContext, AppLifecyclePhase, TestAppContext};
use sheprd_client::{ClientEvent, VersionMismatch, protocol::ClientMessage};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[gpui::test]
fn response_single_poll_key_late_projection_never_sends_to_old_pane(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    let (client, receipts, release) =
        mock_peer::connect_response_first().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        this.navigate_board(
            "sandbox",
            Target::Agent("w2:p2".into()),
            "boot-v1",
            this.terminal.link.generation,
        )
        .unwrap_or_else(|e| panic!("navigate: {e}"));
    });
    loop {
        let received = view.update(cx, |this, _| {
            this.terminal
                .link
                .inbox
                .lock()
                .unwrap_or_else(|e| panic!("lock {e}"))
                .completed_request
                .is_some()
        });
        if received {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    // Precisely one UI poll after the correlated success, with rev 7/p1 still live.
    view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        assert_eq!(
            board::snapshot(&this.terminal.live).map(|s| s.revision),
            Some(7)
        );
        this.refresh_terminal_input();
        this.terminal.input.fresh_gesture();
        if let Some(key) = this.terminal.input.commit("must not reach p1") {
            this.send_terminal(key);
        }
    });
    release.send(()).unwrap_or_else(|e| panic!("release {e}"));
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.navigating.is_none()
                && crate::terminal_input::destination(
                    &this.terminal.live,
                    this.terminal.link.generation,
                )
                .is_some_and(|d| d.target == crate::terminal::InputTarget::Pane("w2:p2".into()))
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        this.refresh_terminal_input();
        this.terminal.input.fresh_gesture();
        let key = this
            .terminal
            .input
            .commit("positive p2")
            .unwrap_or_else(|| panic!("coherent p2 input"));
        this.send_terminal(key);
    });
    let mut saw_request = false;
    loop {
        match receipts.recv_timeout(Duration::from_secs(2)) {
            Ok(ClientMessage::ClientShellEndpointRequest { .. }) => saw_request = true,
            Ok(ClientMessage::ClientShellPaneInput { pane_id, .. }) => {
                assert_eq!(pane_id, "w2:p2", "late projection leaked input to p1");
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("missing positive peer receipt: {e}"),
        }
    }
    assert!(saw_request);
    assert!(receipts.recv_timeout(Duration::from_millis(100)).is_err());
    println!(
        "R4 D1 negative PASS: Response -> single poll -> key -> late projection; peer received no p1 input, positive p2 input received"
    );
}

#[gpui::test]
fn automatic_return_keeps_late_reader_stop_for_selected_and_hidden(cx: &mut TestAppContext) {
    check_late_stop(cx, false);
}
#[gpui::test]
fn injection_right_after_guard_keeps_stop_and_prevents_dial(cx: &mut TestAppContext) {
    check_late_stop(cx, true);
}
fn check_late_stop(cx: &mut TestAppContext, after_guard: bool) {
    for selected in [true, false] {
        let view = cx.new(Smoke::new);
        let (release, wait) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        view.update(cx, |this, cx| {
            this.select_host("selected@invalid.test");
            this.terminal.open = selected;
            let host = if selected {
                "selected@invalid.test"
            } else {
                "hidden@invalid.test"
            };
            let terminal = if selected {
                &mut this.terminal
            } else {
                this.board.hidden.entry(host.into()).or_default()
            };
            terminal.reconnect.begin(host.into());
            assert!(terminal.reconnect.automatic);
            let old = terminal.link.inbox.clone();
            let late = old.clone();
            terminal.link.reader = Some(std::thread::spawn(move || {
                let _ = wait.recv_timeout(Duration::from_secs(5));
                late.lock()
                    .unwrap_or_else(|e| panic!("reader lock {e}"))
                    .apply(ClientEvent::VersionMismatch(
                        VersionMismatch::DaemonOutdated {
                            server_version: Some("R5-late-mismatch".into()),
                        },
                    ));
                let _ = done.send(());
            }));
            if after_guard {
                let reason = terminal.link.stop_reason.clone();
                terminal.link.before_dial = Some(Box::new(move || {
                    let _ = reason.set(VersionMismatch::DaemonOutdated {
                        server_version: Some("R5-after-guard".into()),
                    });
                }));
            }
            this.lifecycle(AppLifecyclePhase::Inactive, cx);
            let generation = if selected {
                this.terminal.link.generation
            } else {
                this.board.hidden[host].link.generation
            };
            this.lifecycle(AppLifecyclePhase::Active, cx);
            let terminal = if selected {
                &this.terminal
            } else {
                &this.board.hidden[host]
            };
            assert_eq!(
                terminal.link.generation, generation,
                "dial began before reader finished"
            );
            assert!(terminal.reconnect.current(host, 1), "extra dial attempt");
            assert!(Arc::ptr_eq(
                &terminal.link.stop_reason,
                &old.lock()
                    .unwrap_or_else(|e| panic!("lock {e}"))
                    .version_mismatch
            ));
        });
        release.send(()).unwrap_or_else(|e| panic!("release {e}"));
        finished
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|e| panic!("finish {e}"));
        // Repeated polling also exercises the pending retry after reader completion.
        for _ in 0..3 {
            view.update(cx, |this, cx| {
                this.poll_terminal(cx);
            });
        }
        view.update(cx, |this, _| {
            let terminal = if selected { &this.terminal }
                else { &this.board.hidden["hidden@invalid.test"] };
            assert!(terminal.link.stop_reason.get().is_some());
            assert!(terminal.live.status.contains("R5-"));
            assert!(terminal.reconnect.current(&terminal.reconnect.target, 1));
            assert!(terminal.link.handle.is_none());
            println!("R3 D1 negative PASS: Automatic selected={selected} after_guard={after_guard}; retained stop, no extra dial");
        });
    }
}

#[test]
fn finished_compatible_reader_releases_dial_permission_without_waiting_on_ui() {
    let mut link = crate::link::Link::default();
    let (release, wait) = mpsc::channel();
    link.reader = Some(std::thread::spawn(move || {
        let _ = wait.recv();
    }));
    link.pause();
    let generation = link.generation;
    assert!(link.prepare_connection().is_none());
    assert_eq!(link.generation, generation);
    release.send(()).unwrap_or_else(|e| panic!("release {e}"));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(next) = link.prepare_connection() {
            assert_eq!(next, generation + 1);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "compatible reader did not release dial permission"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    println!("R3 positive PASS: compatible old reader finishes; deferred dial permission released");
}
