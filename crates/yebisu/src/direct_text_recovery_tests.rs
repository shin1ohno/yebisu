//! SH1-156 D1: ordinary text is not a discarded composition.
//! Call the real key and text adapters without a tap or fresh_gesture reset.
use crate::{app::Smoke, discard_recovery_tests::*, mock_peer};
use gpui::{Context, EntityInputHandler, KeyDownEvent, Keystroke, TestAppContext, Window};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientPaneInputEvent};
use std::time::Duration;

fn printable(this: &mut Smoke, text: &str, window: &mut Window, cx: &mut Context<Smoke>) {
    this.terminal_key_down(
        &KeyDownEvent {
            keystroke: Keystroke {
                key: text.into(),
                key_char: Some(text.into()),
                modifiers: Default::default(),
            },
            is_held: false,
            prefer_character_input: false,
        },
        window,
        cx,
    );
    // SDK's English hardware fallback uses the same insertion as direct text.
    this.replace_text_in_range(None, text, window, cx);
}

#[gpui::test]
fn sh1_156_fresh_herdr1_accepts_printable_and_direct_text(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        this.terminal.test_client = Some(client);
        this.connect_terminal(cx);
    });
    wait_frame(&view, cx, |this| this.terminal.live.connected);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let _ = this.render_terminal(window, cx);
            assert!(this.terminal_focus.is_focused(window));
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(
                !this.terminal.input.requires_gesture(),
                "D1: pristine connection invented a discarded composition"
            );
            assert!(!this.terminal_status().starts_with("Text input paused"));
            this.replace_text_in_range(None, "direct", window, cx);
            printable(this, "a", window, cx);
            key(this, "ctrl-a", false, window, cx);
            key(this, "left", false, window, cx);
            sentinel(this, "w1:p1");
        })
    });
    assert_eq!(
        receive(&wire, "w1:p1"),
        vec![
            ClientPaneInputEvent::TextCommit("direct".into()),
            ClientPaneInputEvent::TextCommit("a".into()),
            expected_key(ClientKeyCode::Char('a'), 2, ClientKeyKind::Press, false),
            expected_key(ClientKeyCode::Left, 0, ClientKeyKind::Press, false),
        ],
        "D1: ordinary input missing or duplicated"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}

#[gpui::test]
fn sh1_156_fresh_core_and_clean_reattach_accept_direct_text(cx: &mut TestAppContext) {
    let host = crate::core_mock::Host::default();
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    for reconnect in [false, true] {
        let (backend, wire) = host.recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, cx| {
            if !reconnect {
                this.select_host("private-core");
            }
            this.connect_core_using(cx, move |_| Ok(backend));
        });
        wait_frame(&view, cx, |this| this.terminal.live.connected);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let _ = this.render_terminal(window, cx);
                assert!(this.terminal_focus.is_focused(window));
                this.terminal.size = None;
                this.refresh_terminal_input();
                assert!(
                    !this.terminal.input.requires_gesture(),
                    "D1: clean core attach fenced direct text"
                );
                this.replace_text_in_range(None, "direct", window, cx);
                printable(this, "a", window, cx);
                key(this, "ctrl-a", false, window, cx);
                key(this, "left", false, window, cx);
                this.replace_text_in_range(None, "END", window, cx);
            })
        });
        let mut received = Vec::new();
        while !received.ends_with(b"END") {
            let (channel, bytes) = wire
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("wire: {e}"));
            assert_eq!(channel, 1);
            received.extend(bytes);
        }
        assert_eq!(
            received, b"directa\x01\x1b[DEND",
            "D1: wrong ordinary core input"
        );
        cx.update(|window, cx| view.update(cx, |this, cx| this.detach_core(window, cx)));
    }
}

#[gpui::test]
fn sh1_156_clean_barrier_drops_pending_text_and_resumes_fresh_text(cx: &mut TestAppContext) {
    barrier(cx, false, false);
}
#[gpui::test]
fn sh1_156_marked_barrier_retains_discard_fence(cx: &mut TestAppContext) {
    barrier(cx, true, false);
    barrier(cx, false, true);
}
fn barrier(cx: &mut TestAppContext, marked_before: bool, marked_during: bool) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, wire, release_projection) =
        mock_peer::connect_response_first().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, client));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            prepare(this, window, cx);
            if marked_before {
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
            }
            this.navigate_board(
                "sandbox",
                crate::board::Target::Agent("w2:p2".into()),
                "boot-v1",
                this.terminal.link.generation,
            )
            .unwrap_or_else(|e| panic!("navigate: {e}"));
            if marked_during {
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
            }
            printable(this, "p", window, cx);
        })
    });
    wait_frame(&view, cx, |this| {
        this.terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .completed_request
            .is_some()
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.terminal.navigating.is_some());
            printable(this, "q", window, cx);
        })
    });
    release_projection
        .send(())
        .unwrap_or_else(|e| panic!("release: {e}"));
    wait_frame(&view, cx, |this| {
        this.terminal.navigating.is_none()
            && this
                .terminal
                .live
                .snapshot
                .as_ref()
                .is_some_and(|s| s.focused_pane_id.as_deref() == Some("w2:p2"))
    });
    let marked = marked_before || marked_during;
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert_eq!(
                this.terminal.input.requires_gesture(),
                marked,
                "D1: barrier must preserve real discard without inventing one"
            );
            if marked {
                this.replace_text_in_range(None, "discarded", window, cx);
            } else {
                printable(this, "b", window, cx);
            }
            key(this, "ctrl-a", false, window, cx);
            key(this, "left", false, window, cx);
            if marked {
                this.replace_text_in_range(None, "discarded", window, cx);
            }
            sentinel(this, "w2:p2");
        })
    });
    let mut expected = Vec::new();
    if !marked {
        expected.push(ClientPaneInputEvent::TextCommit("b".into()));
    }
    expected.extend([
        expected_key(ClientKeyCode::Char('a'), 2, ClientKeyKind::Press, false),
        expected_key(ClientKeyCode::Left, 0, ClientKeyKind::Press, false),
    ]);
    assert_eq!(
        receive(&wire, "w2:p2"),
        expected,
        "D1: pending or discarded text crossed barrier"
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}

#[gpui::test]
fn sh1_156_host_switch_inherits_discarded_and_rejected_marks(cx: &mut TestAppContext) {
    for rejected in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (client, old_wire) =
            mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, _| install(this, client));
        wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                prepare(this, window, cx);
                if rejected {
                    select(this, false, window, cx);
                }
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
                // Discard repeatedly before switching; an empty second detach is not an ack.
                this.terminal.input.detach();
                this.terminal.input.detach();
                this.select_host("other-host");
                this.refresh_terminal_input();
                assert!(
                    this.terminal.input.requires_gesture(),
                    "D1: host switch lost native discard fence"
                );
            })
        });
        let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, _| {
            let generation = this.terminal.link.reset();
            this.terminal.link.install(generation, Ok(client));
        });
        wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let _ = this.render_terminal(window, cx);
                this.terminal.size = None;
                key(this, "ctrl-a", false, window, cx);
                this.replace_text_in_range(None, "discarded", window, cx);
                sentinel(this, "w1:p1");
            })
        });
        assert_eq!(
            receive(&wire, "w1:p1"),
            vec![expected_key(
                ClientKeyCode::Char('a'),
                2,
                ClientKeyKind::Press,
                false
            )]
        );
        assert!(!old_wire.try_iter().any(|m| matches!(
            m,
            sheprd_client::protocol::ClientMessage::ClientShellPaneInput { .. }
        )));
        view.update(cx, |this, _| {
            this.terminal.link.pause();
            for terminal in this.board.hidden.values_mut() {
                terminal.link.pause();
            }
        });
    }
}

#[gpui::test]
fn sh1_156_unmark_and_blur_keep_discarded_text_fenced(cx: &mut TestAppContext) {
    for blurred in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, _| install(this, client));
        wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                prepare(this, window, cx);
                if blurred {
                    window.blur(cx);
                }
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
                if blurred {
                    this.terminal_focus.focus(window, cx);
                } else {
                    this.unmark_text(window, cx);
                }
                this.terminal.input.detach();
                this.refresh_terminal_input();
                key(this, "ctrl-a", false, window, cx);
                this.replace_text_in_range(None, "discarded", window, cx);
                assert!(
                    this.terminal.input.requires_gesture(),
                    "D1: unmark or rejected blur mark lost discard fence"
                );
                sentinel(this, "w1:p1");
            })
        });
        assert_eq!(
            receive(&wire, "w1:p1"),
            vec![expected_key(
                ClientKeyCode::Char('a'),
                2,
                ClientKeyKind::Press,
                false
            )]
        );
        view.update(cx, |this, _| this.terminal.link.pause());
    }
}

#[gpui::test]
fn sh1_156_setup_mark_cannot_commit_into_initial_terminal(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.focus.focus(window, cx);
            this.replace_and_mark_text_in_range(None, "setup-mark", None, window, cx);
            assert!(this.setup_input.marked.is_some());
        })
    });
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, client));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let _ = this.render_terminal(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert!(
                this.terminal.input.requires_gesture(),
                "D1: setup composition lost its discard fence"
            );
            key(this, "ctrl-a", false, window, cx);
            this.replace_text_in_range(None, "setup-mark", window, cx);
            sentinel(this, "w1:p1");
        })
    });
    assert_eq!(
        receive(&wire, "w1:p1"),
        vec![expected_key(
            ClientKeyCode::Char('a'),
            2,
            ClientKeyKind::Press,
            false
        )]
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}
