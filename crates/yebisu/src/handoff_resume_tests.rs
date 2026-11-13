//! Explicit paste/selection exit starts new input; callbacks while selected stay fenced.
use crate::{app::Smoke, mock_peer};
use gpui::{ClipboardItem, EntityInputHandler, MouseButton, TestAppContext, point, px, size};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::time::{Duration, Instant};

#[gpui::test]
fn controlled_retirement_resumes_soft_text_and_ime_after_paste(cx: &mut TestAppContext) {
    for (paste, select) in [(true, true), (false, true), (true, false)] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.simulate_resize(size(px(1200.), px(1000.)));
        let (client, receipts) =
            mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let generation = this.terminal.link.reset();
            this.terminal.link.install(generation, Ok(client));
            this.terminal.open = true;
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.surface.is_some()
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "surface timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
        cx.run_until_parked();
        cx.write_to_clipboard(ClipboardItem::new_string("resume-paste".into()));
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                this.terminal.size = None;
                this.refresh_terminal_input();
                this.terminal.input.fresh_gesture();
                this.replace_and_mark_text_in_range(
                    None,
                    "old-composition",
                    Some(0..3),
                    window,
                    cx,
                );
                assert!(!this.terminal.input.composition.text.is_empty());
                if select {
                    this.toggle_selection();
                    let g = this.terminal.geometry;
                    this.selection_down(
                        &gpui::MouseDownEvent {
                            position: g.grid.origin
                                + point(px(1.5 * g.cell_width), px(2.5 * g.cell_height)),
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Default::default(),
                            first_mouse: false,
                        },
                        window,
                        cx,
                    );
                    assert!(this.handoff.selection.is_some());
                    // A commit from the cancelled composition, before the new gesture, is rejected.
                    this.replace_text_in_range(None, "old-composition", window, cx);
                    assert!(this.terminal.input.requires_gesture());
                }
                if paste {
                    this.start_paste(false, cx);
                    assert!(
                        this.handoff.preparing,
                        "test must exercise queued input during preparation"
                    );
                } else {
                    this.toggle_selection();
                }
                // A gesture alone cannot acknowledge native cancellation.
                this.replace_text_in_range(None, "old-composition-after-resume", window, cx);
                assert!(this.terminal.input.requires_gesture());
                // Controlled SDK boundary only, not native/OS evidence.
                this.controlled_text_retirement();
                this.replace_text_in_range(None, "soft-text", window, cx);
                this.replace_and_mark_text_in_range(None, "かな", Some(0..2), window, cx);
                assert_eq!(this.terminal.input.composition.text, "かな");
                this.replace_text_in_range(None, "仮名", window, cx);
            })
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if view.update(cx, |this, _| {
                // Rendering requests a viewport resize, but this peer intentionally keeps
                // its fixed fixture dimensions. Drain the normal input queue before a
                // direct wire barrier; otherwise the barrier can overtake size-held text.
                this.terminal.size = None;
                this.refresh_terminal_input();
                this.flush_terminal();
                !this.handoff.preparing && this.terminal.input.ready
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "paste timeout");
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
                .unwrap_or_else(|e| panic!("barrier {e}"));
        });
        let mut events = Vec::new();
        'wire: loop {
            let message = receipts
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("wire {e}"));
            if let ClientMessage::ClientShellPaneInput {
                pane_id,
                events: received,
            } = message
            {
                assert_eq!(pane_id, "w1:p1");
                for event in received {
                    if event == ClientPaneInputEvent::Paste("barrier".into()) {
                        break 'wire;
                    }
                    events.push(event);
                }
            }
        }
        let mut expected = Vec::new();
        if paste {
            expected.push(ClientPaneInputEvent::Paste("resume-paste".into()));
        }
        expected.extend([
            ClientPaneInputEvent::TextCommit("soft-text".into()),
            ClientPaneInputEvent::TextCommit("仮名".into()),
        ]);
        assert_eq!(
            events, expected,
            "selection exit paste={paste}: FIFO, no late selected commit"
        );
        view.update(cx, |this, _| this.terminal.link.pause());
        eprintln!(
            "CONTROLLED RETIREMENT PASS paste={paste} select={select}: soft text / IME / FIFO / selected late commit rejected"
        );
    }
}

// SH1-156: use the same adapter methods registered by terminal_view.
#[gpui::test]
#[ignore = "SH1-156: fresh IME remains blocked; pending composition-origin contract"]
fn sh1_156_new_composition_after_discard_without_tap(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "surface timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_and_mark_text_in_range(None, "discard-me", None, window, cx);
            this.terminal.input.discard();
            this.replace_text_in_range(None, "discard-me", window, cx);
            this.replace_and_mark_text_in_range(None, "new-composition", None, window, cx);
            assert_eq!(this.terminal.input.composition.text, "new-composition");
            this.replace_text_in_range(None, "new-commit", window, cx);
        })
    });
    loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire {e}"));
        if let ClientMessage::ClientShellPaneInput { events, .. } = message {
            assert_eq!(
                events,
                vec![ClientPaneInputEvent::TextCommit("new-commit".into())]
            );
            break;
        }
    }
    view.update(cx, |this, _| this.terminal.link.pause());
}

// SH1-156: use the same adapter methods registered by terminal_view.
#[gpui::test]
fn sh1_156_key_does_not_authorize_a_discarded_commit(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "surface timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_and_mark_text_in_range(None, "discard-me", None, window, cx);
            this.terminal.input.discard();
            this.replace_text_in_range(None, "discard-me", window, cx);
            let key = gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("ctrl-a").unwrap_or_else(|e| panic!("key {e}")),
                is_held: false,
                prefer_character_input: false,
            };
            this.terminal_key_down(&key, window, cx);
            assert!(this.terminal.input.requires_gesture());
            // This is indistinguishable from fresh direct text at this adapter.
            this.replace_text_in_range(None, "discard-me", window, cx);
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("handle"))
                .send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste("end".into())],
                )
                .unwrap_or_else(|e| panic!("barrier {e}"));
        })
    });
    let mut received = Vec::new();
    'wire: loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire {e}"));
        if let ClientMessage::ClientShellPaneInput { events, .. } = message {
            for event in events {
                if event == ClientPaneInputEvent::Paste("end".into()) {
                    break 'wire;
                }
                received.push(event);
            }
        }
    }
    assert_eq!(
        received.len(),
        1,
        "only the new hardware key, never the injected discarded commit"
    );
    assert!(!matches!(&received[0], ClientPaneInputEvent::TextCommit(_)));
    eprintln!(
        "SH1-156 app adapter discard fence: {received:?}; native cancellation still unproven"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}
