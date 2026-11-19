//! An accepted retirement acknowledgement requests one GPUI frame, and text
//! input readiness counts as settled only after that frame. The P13 fixture
//! peer's idle bound starts when it is armed (the first fixture tap), not at
//! connect.
#![allow(clippy::unwrap_used)]
use super::retired_forward_tests::{close, install};
use crate::{
    app::Smoke,
    find::retirement::{Ticket, deliver_find_retirement},
    observation::Observation,
};
use gpui::{Bounds, ElementInputHandler, Entity, InputHandler, TestAppContext};
use serde_json::Value;
use sheprd_client::protocol::ClientMessage;
use std::{
    sync::mpsc::{RecvTimeoutError, sync_channel},
    time::{Duration, Instant},
};

/// `begin_find_retirement` answers `None` while the connection reader holds the
/// inbox or the terminal input has not yet synced the destination that close
/// detached; production asks again on the next render. Drain the reader's
/// replies and ask again until the owner is whole, or fail at the bound.
fn begin_retirement(view: &Entity<Smoke>, cx: &mut gpui::App) -> Ticket {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let ticket = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.begin_find_retirement(40)
        });
        if let Some(ticket) = ticket {
            return ticket;
        }
        assert!(
            Instant::now() < deadline,
            "retirement owner never became whole"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn accepted_retirement_ack_draws_the_frame_that_settles_text_input(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let _receipts = install(&view, cx);
    let (records, recorded) = sync_channel::<Value>(64);
    view.update(cx, |this, _| {
        this.observation = Observation::injected(records)
    });
    cx.update(|window, cx| {
        window.activate_window();
        close(&view, window, cx);
    });
    // close() ended in a draw with the fence closed: GPUI read FocusLost
    // (input.rs:98), and nothing has acknowledged the retirement.
    cx.update(|window, cx| {
        assert!(view.read(cx).find_handoff.paused);
        assert!(!view.read(cx).find_input_settled());
        let mut terminal = ElementInputHandler::new(Bounds::default(), view.clone());
        assert!(!terminal.accepts_text_input(window, cx));
    });

    // Each ack is delivered from inside an update, so the immediate path
    // finds the app borrowed and the deferred task acknowledges, as on the
    // Mac mini (attempt 8, turn 2).
    let deliver = |sessions: (u64, u64), cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            let ticket = begin_retirement(&view, cx);
            deliver_find_retirement(
                window.to_async(cx),
                view.downgrade(),
                window.window_handle(),
                ticket,
                sessions,
            );
        });
        cx.run_until_parked();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let record = recorded
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if record["kind"] == "p13-native-retirement" {
                assert_eq!(record["deferred"], true, "ack was not deferred");
                return record["accepted"] == true;
            }
        }
    };

    // A rejected ack (replacement is not retired + 1) keeps the fence closed;
    // later frames do not settle readiness.
    assert!(!deliver((40, 43), cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(view.read(cx).find_handoff.paused);
        assert!(!view.read(cx).find_input_settled());
    });

    // The accepted ack opens the fence and requests a frame; after that frame
    // the terminal handler GPUI registered accepts text input.
    let before = cx.update(|_, cx| view.read(cx).find_handoff.frames);
    assert!(deliver((40, 41), cx));
    cx.update(|window, cx| {
        let this = view.read(cx);
        assert!(!this.find_handoff.paused, "fence survived the accepted ack");
        assert_ne!(this.find_handoff.frames, before, "no frame after the ack");
        assert!(
            this.find_input_settled(),
            "readiness not settled after the frame"
        );
        let mut terminal = ElementInputHandler::new(Bounds::default(), view.clone());
        assert!(terminal.accepts_text_input(window, cx));
    });

    // Fencing again (search reopened and closed) clears the settled state.
    cx.update(|window, cx| {
        close(&view, window, cx);
    });
    cx.update(|_, cx| assert!(!view.read(cx).find_input_settled()));
}

fn request(client: &sheprd_client::Client, label: &str) {
    client
        .handle
        .request(
            "boot-v1",
            sheprd_client::Method::PaneScroll,
            serde_json::json!({"pane_id":"w1:p1","offset_from_bottom":100,"p13_settle":label}),
        )
        .unwrap();
}

/// Waits for `label`'s request at the peer; the peer thread is alive.
fn reached(received: &std::sync::mpsc::Receiver<ClientMessage>, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let message = received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e| panic!("{label} never reached the peer: {e}"));
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = message {
            let request: Value = serde_json::from_str(&request).unwrap();
            if request["params"]["p13_settle"] == label {
                return;
            }
        }
    }
}

#[test]
fn fixture_peer_idle_bound_starts_at_the_first_tap() {
    let limit = Duration::from_millis(300);
    let (client, received, idle) = crate::find::peer::connect_idle(limit).unwrap();
    // Unarmed (no fixture tap yet): three bounds of silence keep the link.
    std::thread::sleep(limit * 3);
    assert!(
        !client.handle.is_disconnected(),
        "link dropped before the first tap"
    );
    request(&client, "before-tap");
    reached(&received, "before-tap");

    // The first tap arms the bound; activity inside it keeps the link.
    idle.arm();
    let armed = Instant::now();
    std::thread::sleep(limit / 2);
    // The peer stamps its last activity when it receives the request, which is
    // after this instant and before `reached` returns; stamping here, not after
    // `reached`, keeps the lower bound below the peer's own clock.
    let sent = Instant::now();
    request(&client, "after-tap");
    reached(&received, "after-tap");
    // A later tap does not restart the bound.
    idle.arm();

    // Silence past the bound ends the link: the peer thread exits, dropping
    // its receipt sender, and the client sees the disconnect.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match received.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("armed idle bound never ended the link"),
        }
    }
    let ended = Instant::now();
    assert!(
        ended.duration_since(sent) >= limit,
        "ended inside the bound"
    );
    assert!(ended.duration_since(armed) >= limit + limit / 2);
    while !client.handle.is_disconnected() {
        assert!(
            Instant::now() < deadline,
            "client never saw the dropped link"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
