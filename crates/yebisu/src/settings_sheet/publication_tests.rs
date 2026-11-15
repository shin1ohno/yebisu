//! Real publication workers and receiving peers; no live host or credential calls.
use crate::app::Smoke;
use gpui::{ClipboardItem, EntityInputHandler, TestAppContext};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[gpui::test]
fn f19_h1_modal_cancels_after_validation_before_actual_publication(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, wire) = crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("{e}"));
    view.update(cx, |this, _| {
        this.select_host("private-peer");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    let (gate, control) = crate::handoff::AsyncPublicationGate::new();
    cx.write_to_clipboard(ClipboardItem::new_string(
        "F19_FORBIDDEN_PUBLICATION".into(),
    ));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "before", window, cx);
            this.handoff.publication_gate = Some(gate);
            this.start_paste(false, cx);
        })
    });
    loop {
        cx.run_until_parked();
        if control.entered() {
            break;
        }
        assert!(Instant::now() < until, "publication seam never reached");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.handoff.preparing);
            this.begin_settings_modal(window, cx);
        })
    });
    control.release();
    while view.read_with(cx, |this, _| this.handoff.workers != 0) {
        cx.run_until_parked();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    view.update(cx, |this, _| {
        this.terminal
            .link
            .handle
            .as_ref()
            .unwrap_or_else(|| panic!("handle"))
            .send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste("barrier".into())],
            )
            .unwrap_or_else(|e| panic!("{e}"));
    });
    let mut events = Vec::new();
    'receive: loop {
        if let ClientMessage::ClientShellPaneInput {
            pane_id,
            events: batch,
        } = wire
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("{e}"))
        {
            assert_eq!(pane_id, "w1:p1");
            for event in batch {
                if event == ClientPaneInputEvent::Paste("barrier".into()) {
                    break 'receive;
                }
                events.push(event);
            }
        }
    }
    assert_eq!(events, [ClientPaneInputEvent::TextCommit("before".into())]);
}

#[gpui::test]
fn f19_core_queued_paste_cancelled_by_modal_before_worker_dispatch(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (backend, wire) = crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("{e}"));
    view.update(cx, |this, cx| {
        this.select_host("private-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.connected
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    let (entered, arrived) = mpsc::sync_channel(1);
    let (release, resume) = mpsc::sync_channel(1);
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"))
            .send(crate::core::Command::PauseInput(
                crate::handoff::PublicationGate {
                    entered,
                    release: resume,
                },
            ))
            .unwrap_or_else(|e| panic!("{e}"));
    });
    arrived
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|e| panic!("{e}"));
    cx.write_to_clipboard(ClipboardItem::new_string("F19_CORE_FORBIDDEN".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.start_paste(false, cx);
            assert_eq!(this.handoff.clipboard_reads, 1);
            this.replace_and_mark_text_in_range(None, "old composition", Some(0..1), window, cx);
            this.begin_settings_modal(window, cx);
            assert!(this.terminal.input.composition.text.is_empty());
            this.replace_text_in_range(None, "modal leak", window, cx);
            this.end_settings_modal(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal_focus.focus(window, cx);
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "after-barrier", window, cx);
        })
    });
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"after-barrier") {
        let (_, payload) = wire
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("{e}"));
        bytes.extend(payload);
    }
    assert_eq!(bytes, b"after-barrier");
    // This exercises the consumer queue. SDK transport already accepted before
    // invalidation still needs a separate final-writer dependency test.
}
