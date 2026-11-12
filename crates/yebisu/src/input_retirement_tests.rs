//! Controlled consumer callback contract, not UIKit or queued-OS-input evidence.
use super::*;
use crate::{discard_recovery_tests as adapter, mock_peer};
use gpui::{AppContext, EntityInputHandler, TestAppContext};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::time::Duration;

fn discarded(this: &mut Smoke) {
    this.terminal.open = true;
    this.terminal.live.connected = true;
    this.terminal.input.reject_mark("old😀");
}

#[gpui::test]
fn retirement_rejects_each_changed_owner_and_newer_mark(cx: &mut TestAppContext) {
    for change in 0..8 {
        let view = cx.new(Smoke::new);
        view.update(cx, |this, _| {
            discarded(this);
            let ticket = this
                .begin_text_retirement(7)
                .unwrap_or_else(|| panic!("ticket"));
            match change {
                0 => this.terminal.reconnect.target.push_str("other"),
                1 => this.terminal.link.generation += 1,
                2 => this.board.selection += 1,
                3 => this.terminal.input.reject_mark("newer😀"),
                4 => this.terminal.open = false,
                5 => this.terminal.reconnect.suspended = true,
                6 => this.terminal.core_mode = true,
                _ => {
                    this.terminal.input.destination = Some(crate::terminal_input::Destination {
                        generation: 1,
                        epoch: 1,
                        boot: "other".into(),
                        target: crate::terminal::InputTarget::Pane("p2".into()),
                    })
                }
            }
            assert!(
                !this.complete_text_retirement(&ticket, 7, 8),
                "changed owner {change}"
            );
            assert!(this.terminal.input.requires_gesture());
            assert!(this.terminal.input.commit("old😀").is_none());
        });
    }
}

#[gpui::test]
fn retirement_registers_before_callback_and_consumes_only_matching_ticket(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        discarded(this);
        let old = this
            .begin_text_retirement(7)
            .unwrap_or_else(|| panic!("old ticket"));
        let current = this
            .begin_text_retirement(8)
            .unwrap_or_else(|| panic!("current ticket"));
        assert!(!this.complete_text_retirement(&old, 7, 8));
        assert!(this.terminal.input.requires_gesture());
        // The consumer callback can precede the return of the SDK receipt.
        assert!(this.complete_text_retirement(&current, 8, 9));
        assert!(!this.complete_text_retirement(&current, 8, 9));
        this.terminal.input.reject_mark("new discard");
        assert!(!this.complete_text_retirement(&old, 7, 8));
        assert!(this.terminal.input.requires_gesture());
    });
}

#[gpui::test]
fn retirement_wrong_native_ids_and_refusal_leave_fence(cx: &mut TestAppContext) {
    for ids in [(6, 7), (7, 7), (7, 9)] {
        let view = cx.new(Smoke::new);
        view.update(cx, |this, _| {
            discarded(this);
            let ticket = this
                .begin_text_retirement(7)
                .unwrap_or_else(|| panic!("ticket"));
            assert!(!this.complete_text_retirement(&ticket, ids.0, ids.1));
            assert!(this.terminal.input.requires_gesture());
        });
    }
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        discarded(this);
        let ticket = this
            .begin_text_retirement(7)
            .unwrap_or_else(|| panic!("ticket"));
        this.refuse_text_retirement(&ticket, "NativeCallbackInFlight");
        assert!(!this.complete_text_retirement(&ticket, 7, 8));
        assert!(this.terminal.input.requires_gesture());
        this.retirement.serial = u64::MAX;
        assert!(this.begin_text_retirement(7).is_none());
        assert!(this.terminal.input.requires_gesture());
    });
}

#[gpui::test]
fn controlled_retirement_sends_fresh_same_text_once_to_real_peer(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| adapter::install(this, client));
    adapter::wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            adapter::prepare(this, window, cx);
            this.replace_and_mark_text_in_range(None, "same😀", None, window, cx);
            this.unmark_text(window, cx);
            this.replace_text_in_range(None, "same😀", window, cx);
            assert!(this.terminal.input.requires_gesture());
            this.controlled_text_retirement();
            this.replace_and_mark_text_in_range(None, "same😀", None, window, cx);
            assert_eq!(this.terminal.input.composition.text, "same😀");
            this.replace_text_in_range(None, "same😀", window, cx);
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("handle"))
                .send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste("retirement-end".into())],
                )
                .unwrap_or_else(|e| panic!("sentinel {e}"));
        })
    });
    let mut events = Vec::new();
    'drain: loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire {e}"));
        if let ClientMessage::ClientShellPaneInput {
            pane_id,
            events: batch,
        } = message
        {
            assert_eq!(pane_id, "w1:p1");
            for event in batch {
                if event == ClientPaneInputEvent::Paste("retirement-end".into()) {
                    break 'drain;
                }
                events.push(event);
            }
        }
    }
    assert_eq!(events, [ClientPaneInputEvent::TextCommit("same😀".into())]);
    view.update(cx, |this, _| this.terminal.link.pause());
}

#[gpui::test]
fn clean_disconnected_and_suspended_owners_cannot_acknowledge_retirement(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.terminal.open = true;
        this.terminal.live.connected = true;
        assert!(this.begin_text_retirement(7).is_none());
        discarded(this);
        this.terminal.live.connected = false;
        this.terminal.needs_focus = false;
        assert!(this.begin_text_retirement(7).is_none());
        this.terminal.needs_focus = true;
        this.terminal.reconnect.suspended = true;
        assert!(this.begin_text_retirement(7).is_none());
        assert!(this.terminal.input.requires_gesture());
        this.terminal.reconnect.suspended = false;
        assert!(this.begin_text_retirement(7).is_some());
        assert!(
            this.terminal.input.requires_gesture(),
            "pending is not acknowledgement"
        );
    });
}
