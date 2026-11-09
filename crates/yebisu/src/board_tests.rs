use super::*;
use sheprd_client::ClientEvent;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn live() -> Live {
    let mut live = Live::default();
    live.connected = true;
    live.apply(ClientEvent::Snapshot(Arc::new(
        serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}")),
    )));
    live
}
#[test]
fn status_comes_only_from_latest_daemon_snapshot() {
    let mut live = live();
    let initial = rows(&live);
    assert_eq!(initial.len(), 2);
    assert_eq!(initial[0].status, AgentStatus::Unknown);
    assert_eq!(initial[1].status, AgentStatus::Blocked);
    assert!(initial[1].label.contains("[Claude]"));
    live.apply(ClientEvent::Surface(Arc::new(crate::mock_peer::surface(7))));
    let mut newer = snapshot(&live)
        .cloned()
        .unwrap_or_else(|| panic!("snapshot"));
    newer.revision += 1;
    newer.agents[0].agent_status = AgentStatus::Done;
    live.apply(ClientEvent::Snapshot(Arc::new(newer)));
    assert!(live.pending_snapshot.is_some());
    assert_eq!(rows(&live)[1].status, AgentStatus::Done);
    // Arbitrary terminal text never changes the board.
    live.status = "Working Done Blocked".into();
    assert_eq!(rows(&live)[1].status, AgentStatus::Done);
}
#[gpui::test]
fn hidden_hosts_drain_and_old_host_update_cannot_replace_selection(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal
            .link
            .install(generation, crate::mock_peer::connect_burst(400));
        this.select_host("B");
        let generation = this.terminal.link.reset();
        this.terminal
            .link
            .install(generation, crate::mock_peer::connect_burst(300));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let ready = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            let a = &this.board.hidden["A"];
            let b = &this.terminal;
            a.live
                .surface
                .as_ref()
                .is_some_and(|s| s.surface_revision == 400)
                && b.live
                    .surface
                    .as_ref()
                    .is_some_and(|s| s.surface_revision == 300)
        });
        if ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "hidden event receiver stopped draining"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        let mut late = live();
        late.status = "A late update".into();
        late.dirty = true;
        *this.board.hidden["A"]
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| panic!("lock: {e}")) = late;
        this.poll_terminal(cx);
        assert_eq!(this.terminal.reconnect.target, "B");
        assert_ne!(this.terminal.live.status, "A late update");
        assert_eq!(this.board.hidden["A"].live.status, "A late update");
        this.select_host("A");
        assert_eq!(this.terminal.live.status, "A late update");
        assert_eq!(
            this.board.hidden["B"]
                .live
                .surface
                .as_ref()
                .map(|s| s.surface_revision),
            Some(300)
        );
    });
}
#[gpui::test]
fn navigation_uses_endpoint_api_and_refuses_stale_target(cx: &mut gpui::TestAppContext) {
    check_navigation(
        cx,
        Target::Agent("w2:p2".into()),
        "pane.focus",
        "pane_id",
        "w2:p2",
    );
}
#[gpui::test]
fn space_navigation_uses_endpoint_api(cx: &mut gpui::TestAppContext) {
    check_navigation(
        cx,
        Target::Space("w2".into()),
        "workspace.focus",
        "workspace_id",
        "w2",
    );
}
fn check_navigation(
    cx: &mut gpui::TestAppContext,
    target: Target,
    method: &str,
    field: &str,
    id: &str,
) {
    let view = cx.new(Smoke::new);
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            snapshot(&this.terminal.live).is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        assert!(
            this.navigate_board(
                "A",
                Target::Agent("missing".into()),
                "boot-v1",
                this.terminal.link.generation
            )
            .is_err()
        );
        assert!(
            this.navigate_board("A", target, "boot-v1", this.terminal.link.generation)
                .is_ok()
        );
        assert!(this.terminal.navigating.is_some());
        assert!(
            this.navigate_board(
                "A",
                Target::Space("w1".into()),
                "boot-v1",
                this.terminal.link.generation
            )
            .is_err()
        );
    });
    loop {
        let message = receipts
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e| panic!("receipt: {e}"));
        match message {
            sheprd_client::protocol::ClientMessage::ClientShellEndpointRequest {
                request, ..
            } => {
                let value: serde_json::Value =
                    serde_json::from_str(&request).unwrap_or_else(|e| panic!("request: {e}"));
                assert_eq!(value["method"], method);
                assert_eq!(value["params"][field], id);
                break;
            }
            sheprd_client::protocol::ClientMessage::ClientShellPaneInput { .. } => {
                panic!("navigation synthesized terminal input")
            }
            _ => {}
        }
    }
    loop {
        let moved = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            let state = &this.terminal.live;
            snapshot(state).is_some_and(|s| s.focused_pane_id.as_deref() == Some("w2:p2"))
                && state
                    .surface
                    .as_ref()
                    .is_some_and(|s| s.frame.cells[0].symbol == "2")
                && crate::terminal_input::destination(state, this.terminal.link.generation)
                    .is_some_and(|d| d.target == crate::terminal::InputTarget::Pane("w2:p2".into()))
                && this.terminal.navigating.is_none()
        });
        if moved {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "focus, surface and input destination did not move"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn board_peer_updates_without_render_or_selection() {
    let mut link = crate::link::Link::default();
    let generation = link.reset();
    link.install(generation, crate::mock_peer::connect_board());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(update) = link.take_update()
            && snapshot(&update).is_some_and(|s| s.revision >= 10)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "off-screen snapshot stream stalled"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui::test]
fn host_switch_drops_queued_input_and_late_composition(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.select_host("A");
        let mut state = live();
        state.apply(ClientEvent::Surface(Arc::new(crate::mock_peer::surface(7))));
        this.terminal.input.sync(&state, 1, None);
        this.terminal.input.fresh_gesture();
        let event = this
            .terminal
            .input
            .commit("queued")
            .unwrap_or_else(|| panic!("input"));
        assert!(this.terminal.input.queue(event));
        this.terminal.input.mark("old IME", None);
        this.select_host("B");
        this.select_host("A");
        this.terminal.input.sync(&state, 1, None);
        assert!(this.terminal.input.next().is_none());
        assert!(this.terminal.input.composition.text.is_empty());
        assert!(this.terminal.input.commit("late IME").is_none());
        this.terminal.input.fresh_gesture();
        assert!(this.terminal.input.commit("fresh").is_some());
    });
}
#[test]
fn navigation_waits_for_matching_focus_and_new_revision() {
    let mut terminal = TerminalView {
        live: live(),
        navigating: Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Agent("w1:p2".into()),
            request: "request-1".into(),
        }),
        ..Default::default()
    };
    terminal.live.status = "Connected".into();
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_some());
    let mut next = snapshot(&terminal.live)
        .cloned()
        .unwrap_or_else(|| panic!("snapshot"));
    next.revision = 8;
    terminal
        .live
        .apply(ClientEvent::Snapshot(Arc::new(next.clone())));
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_some());
    next.revision = 9;
    next.focused_pane_id = Some("w1:p2".into());
    terminal.live.apply(ClientEvent::Snapshot(Arc::new(next)));
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_none());
}

#[gpui::test]
fn navigation_barrier_discards_input_before_daemon_ack(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.select_host("A");
        let mut state = live();
        state.apply(ClientEvent::Surface(Arc::new(crate::mock_peer::surface(7))));
        *this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| panic!("lock: {e}")) = state.clone();
        this.terminal.input.sync(&state, 0, None);
        this.terminal.input.fresh_gesture();
        let queued = this
            .terminal
            .input
            .commit("old")
            .unwrap_or_else(|| panic!("event"));
        this.terminal.input.queue(queued);
        this.terminal.navigating = Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Agent("w1:p2".into()),
            request: "request-1".into(),
        });
        this.refresh_terminal_input();
        this.terminal.input.fresh_gesture();
        assert!(this.terminal.input.commit("while pending").is_none());
        this.terminal.navigating = None;
        this.refresh_terminal_input();
        assert!(this.terminal.input.next().is_none());
    });
}

#[gpui::test]
fn background_retires_hidden_inbox_without_replaying_it(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.select_host("A");
        this.terminal.reconnect.automatic = false;
        let old = this.terminal.link.inbox.clone();
        this.select_host("B");
        this.terminal.reconnect.automatic = false;
        this.lifecycle(gpui::AppLifecyclePhase::Background, cx);
        let mut stale = live();
        stale.status = "retired A".into();
        stale.dirty = true;
        *old.lock().unwrap_or_else(|e| panic!("lock: {e}")) = stale;
        this.lifecycle(gpui::AppLifecyclePhase::Active, cx);
        this.poll_terminal(cx);
        assert!(!Arc::ptr_eq(&old, &this.board.hidden["A"].link.inbox));
        assert_ne!(this.board.hidden["A"].live.status, "retired A");
        assert_eq!(this.terminal.reconnect.target, "B");
    });
}

#[test]
fn focus_noop_waits_for_correlated_ack_not_an_old_error() {
    let mut state = live();
    state.status = "Previous unrelated error".into();
    state.pending_request = Some("request-1".into());
    let mut terminal = TerminalView {
        live: state,
        navigating: Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Agent("w1:p1".into()),
            request: "request-1".into(),
        }),
        ..Default::default()
    };
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_some());
    terminal.live.apply(ClientEvent::Response {
        request_id: "old-request".into(),
        response: serde_json::json!({"result":{}}),
    });
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_some());
    terminal.live.apply(ClientEvent::Response {
        request_id: "request-1".into(),
        response: serde_json::json!({"result":{}}),
    });
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_none());
}
#[test]
fn api_error_releases_barrier_and_is_visible() {
    let mut state = live();
    state.pending_request = Some("request-1".into());
    let mut terminal = TerminalView {
        live: state,
        navigating: Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Agent("w1:p2".into()),
            request: "request-1".into(),
        }),
        ..Default::default()
    };
    terminal.live.apply(ClientEvent::Response {
        request_id: "request-1".into(),
        response: serde_json::json!({"error":"missing target"}),
    });
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_none());
    assert!(terminal.live.status.contains("missing target"));
}

#[test]
fn successful_move_then_other_focus_before_single_poll_releases_input() {
    let link = crate::link::Link::default();
    let mut state = live();
    state.pending_request = Some("move".into());
    let mut moved = snapshot(&state)
        .cloned()
        .unwrap_or_else(|| panic!("snapshot"));
    moved.revision = 8;
    moved.focused_pane_id = Some("w1:p2".into());
    state.apply(ClientEvent::Snapshot(Arc::new(moved.clone())));
    state.apply(ClientEvent::Response {
        request_id: "unrelated".into(),
        response: serde_json::json!({"result":{}}),
    });
    assert!(state.completed_request.is_none());
    state.apply(ClientEvent::Response {
        request_id: "move".into(),
        response: serde_json::json!({"result":{}}),
    });
    moved.revision = 9;
    moved.focused_pane_id = Some("w1:p3".into());
    state.apply(ClientEvent::Snapshot(Arc::new(moved)));
    state.apply(ClientEvent::Surface(Arc::new(crate::mock_peer::surface(9))));
    *link.inbox.lock().unwrap_or_else(|e| panic!("lock: {e}")) = state;
    let mut terminal = TerminalView {
        live: link.take_update().unwrap_or_else(|| panic!("update")),
        navigating: Some(Navigation {
            boot: "boot-v1".into(),
            revision: 7,
            target: Target::Agent("w1:p2".into()),
            request: "move".into(),
        }),
        ..Default::default()
    };
    finish_navigation(&mut terminal);
    assert!(terminal.navigating.is_none());
    terminal.input.sync(&terminal.live, 0, None);
    terminal.input.fresh_gesture();
    assert!(terminal.input.commit("resumed").is_some());
}

#[gpui::test]
fn stale_row_same_host_and_pane_sends_no_request(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            snapshot(&this.terminal.live).is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        let generation = this.terminal.link.generation;
        let mut state = live();
        let mut changed = snapshot(&state)
            .cloned()
            .unwrap_or_else(|| panic!("snapshot"));
        changed.boot_id = "boot-B".into();
        state = Live::default();
        state.connected = true;
        state.apply(ClientEvent::Snapshot(Arc::new(changed)));
        *this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| panic!("lock: {e}")) = state;
        assert!(
            this.navigate_board("A", Target::Agent("w1:p1".into()), "boot-v1", generation)
                .is_err()
        );
        assert!(
            this.navigate_board(
                "A",
                Target::Agent("w1:p1".into()),
                "boot-B",
                generation.wrapping_sub(1)
            )
            .is_err()
        );
    });
    assert!(
        receipts.recv_timeout(Duration::from_millis(150)).is_err(),
        "stale row sent an API request"
    );
}

#[test]
fn peer_faults_cannot_pass_move_check() {
    for fault in ["ignore", "wrong", "stale"] {
        let mut link = crate::link::Link::default();
        let generation = link.reset();
        link.install(generation, crate::mock_peer::connect_fault(fault));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if link.take_update().is_some_and(|s| s.surface.is_some()) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let handle = link.handle.as_ref().unwrap_or_else(|| panic!("handle"));
        let request = handle
            .focus_pane("boot-v1", "w2:p2")
            .unwrap_or_else(|e| panic!("focus: {e}"));
        link.inbox
            .lock()
            .unwrap_or_else(|e| panic!("lock: {e}"))
            .pending_request = Some(request);
        loop {
            let ready = {
                let state = link.inbox.lock().unwrap_or_else(|e| panic!("lock: {e}"));
                if fault == "stale" {
                    !state.connected && state.status.contains("response boot mismatch")
                } else {
                    state
                        .snapshot
                        .as_ref()
                        .zip(state.surface.as_ref())
                        .is_some_and(|(s, f)| s.revision > 7 && crate::live::coherent(s, f))
                }
            };
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fault {fault} did not reach receiver"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let state = link.inbox.lock().unwrap_or_else(|e| panic!("lock: {e}"));
        let moved = snapshot(&state).is_some_and(|s| {
            s.boot_id == "boot-v1" && s.focused_pane_id.as_deref() == Some("w2:p2")
        }) && state
            .surface
            .as_ref()
            .is_some_and(|s| s.frame.cells[0].symbol == "2")
            && crate::terminal_input::destination(&state, generation)
                .is_some_and(|d| d.target == crate::terminal::InputTarget::Pane("w2:p2".into()));
        assert!(!moved, "fault {fault} passed move check");
        let focused = snapshot(&state).and_then(|s| s.focused_pane_id.as_deref());
        assert_eq!(
            focused,
            Some(if fault == "wrong" { "w1:p3" } else { "w1:p1" })
        );
        println!(
            "D3 negative {fault}: move check rejected peer; focus={focused:?}, status={}",
            state.status
        );
    }
}
