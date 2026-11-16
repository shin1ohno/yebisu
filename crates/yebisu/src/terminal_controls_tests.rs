//! Terminal input keeps its order relative to later requests while the
//! connection reader or the core worker holds the mailbox (P13 D5), a target
//! change published meanwhile still discards it, and a paste during such a
//! publication is admitted instead of refused (L14).
#![allow(clippy::unwrap_used)]
use crate::{
    app::Smoke,
    core_mock::{self, paste::Probe, transport::Control},
    terminal_input::GeneratedInput,
};
use gpui::{ClipboardItem, Entity, TestAppContext};
use sheprd_client::{
    ClientEvent,
    protocol::{ClientMessage, ClientPaneInputEvent},
};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

#[track_caller]
fn wait(view: &Entity<Smoke>, cx: &mut TestAppContext, mut ready: impl FnMut(&mut Smoke) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        crate::core::paste_test_support::pump();
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            ready(this)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "condition failed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A Herdr 1 link to the P13 fixture peer whose input is attached and ready.
fn link_peer(view: &Entity<Smoke>, cx: &mut TestAppContext) -> Receiver<ClientMessage> {
    let (client, receipts) = crate::find::peer::connect().unwrap();
    view.update(cx, |this, _| {
        this.select_host("input-order");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait(view, cx, |this| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.terminal.input.ready && this.terminal.link.inbox.try_lock().is_ok()
    });
    receipts
}

/// The UI observes a ready target on the frame before the gesture. A draw
/// resizes the request to the test window, which the fixture surface does
/// not match, so observe it in the same update that sends.
fn observe_ready_target(this: &mut Smoke) {
    this.terminal.size = None;
    this.refresh_terminal_input();
    this.sync_handoff();
    assert!(
        this.terminal.input.ready && this.handoff.stamp.is_some(),
        "no ready target before the gesture: destination={:?}",
        this.terminal.input.destination
    );
}

/// A request on the same ordered writer as input, like the P13 probe barrier.
fn barrier(this: &Smoke, boot: &str, label: &str) {
    this.terminal
        .link
        .handle
        .as_ref()
        .unwrap()
        .request(
            boot,
            sheprd_client::Method::PaneScroll,
            serde_json::json!({"pane_id":"w1:p1","offset_from_bottom":0,"p13_barrier":label}),
        )
        .unwrap();
}

/// Terminal input the receiving peer saw before `label`'s barrier.
fn before_barrier(receipts: &Receiver<ClientMessage>, label: &str) -> Vec<ClientPaneInputEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    loop {
        match receipts
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            ClientMessage::ClientShellPaneInput {
                pane_id,
                events: next,
            } => {
                assert_eq!(pane_id, "w1:p1", "input reached a wrong pane");
                events.extend(next);
            }
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                if request["params"]["p13_barrier"] == label {
                    return events;
                }
            }
            _ => {}
        }
    }
}

#[gpui::test]
fn input_sent_while_the_reader_publishes_stays_ahead_of_a_later_request(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let receipts = link_peer(&view, cx);
    view.update(cx, |this, _| {
        observe_ready_target(this);
        let inbox = this.terminal.link.inbox.clone();
        // The reader holds the mailbox while it applies an event, as it did
        // with the scroll surface for the previous barrier on the Mac mini.
        let publishing = inbox.lock().unwrap();
        let destination = this.terminal.input.destination.clone().unwrap();
        this.send_terminal(GeneratedInput {
            destination: destination.clone(),
            event: ClientPaneInputEvent::TextCommit("typed".into()),
        });
        barrier(this, &destination.boot, "after-typed");
        drop(publishing);
        assert!(
            this.terminal.input.ready,
            "a busy mailbox made input unready"
        );
        assert!(
            !this.terminal.live.status.contains("input discarded"),
            "{}",
            this.terminal.live.status
        );
    });
    assert_eq!(
        before_barrier(&receipts, "after-typed"),
        vec![ClientPaneInputEvent::TextCommit("typed".into())],
        "input queued before the request reached the peer after it"
    );
}

#[gpui::test]
fn a_target_change_published_during_contention_still_discards_input(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let receipts = link_peer(&view, cx);
    view.update(cx, |this, _| {
        observe_ready_target(this);
        let inbox = this.terminal.link.inbox.clone();
        let mut publishing = inbox.lock().unwrap();
        let destination = this.terminal.input.destination.clone().unwrap();
        // The reader completes a publication that ends this destination and
        // keeps the mailbox; the UI has not observed the change.
        publishing.apply(ClientEvent::Disconnected {
            reason: "test".into(),
            ssh: None,
        });
        this.send_terminal(GeneratedInput {
            destination: destination.clone(),
            event: ClientPaneInputEvent::TextCommit("stale".into()),
        });
        barrier(this, &destination.boot, "after-stale");
        drop(publishing);
        assert!(
            this.terminal
                .live
                .status
                .ends_with("Input target changed; input discarded"),
            "{}",
            this.terminal.live.status
        );
    });
    assert_eq!(
        before_barrier(&receipts, "after-stale"),
        Vec::<ClientPaneInputEvent>::new(),
        "input reached a target the reader had already retired"
    );
}

#[gpui::test]
fn paste_during_a_core_publication_is_admitted(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    let (backend, _) = core_mock::controlled(probe.clone(), Control::default()).unwrap();
    view.update(cx, |this, cx| {
        this.setup_input.text = "l14-host".into();
        this.terminal.open = false;
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    // The UI observed the target on an earlier frame, as rendering does.
    wait(&view, cx, |this| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.sync_handoff();
        this.terminal.live.connected
            && this.terminal.input.ready
            && this.handoff.stamp.is_some()
            && this.terminal.link.inbox.try_lock().is_ok()
    });
    cx.write_to_clipboard(ClipboardItem::new_string("l14-paste".into()));
    let (reads, status) = view.update(cx, |this, cx| {
        observe_ready_target(this);
        let inbox = this.terminal.link.inbox.clone();
        // The core worker holds the mailbox while it publishes an echo
        // (core/worker.rs), which start_paste used to read as "frame pending".
        let publishing = inbox.lock().unwrap();
        let before = this.handoff.clipboard_reads;
        this.start_paste(false, cx);
        drop(publishing);
        (
            this.handoff.clipboard_reads - before,
            this.terminal.live.status.clone(),
        )
    });
    assert_eq!(reads, 1, "paste refused during a publication: {status}");
    assert!(!status.contains("Paste refused"), "{status}");
    wait(&view, cx, |_| {
        probe
            .lock()
            .inputs
            .iter()
            .any(|(_, bytes)| bytes == b"l14-paste")
    });
}
