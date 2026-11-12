//! Connection controls and status use the production handlers, with private transports.
use crate::{app::Smoke, discard_recovery_tests::*, mock_peer};
use gpui::{EntityInputHandler, TestAppContext};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientPaneInputEvent};
use std::time::Duration;

#[gpui::test]
fn sh1_156_reconnect_control_restores_focus_and_keys(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, _) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, client));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            prepare(this, window, cx);
            select(this, true, window, cx);
            window.blur(cx);
        })
    });
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.terminal.test_client = Some(client);
        this.connect_terminal(cx);
    });
    wait_frame(&view, cx, |this| {
        this.terminal.live.connected && this.terminal.test_client.is_none()
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Render must restore focus; the test never taps or manually focuses here.
            let _ = this.render_terminal(window, cx);
            assert!(this.terminal_focus.is_focused(window));
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.sync_handoff();
            assert!(!this.handoff.select_mode);
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
    view.update(cx, |this, _| this.terminal.link.pause());
}

#[gpui::test]
fn sh1_156_core_detach_reattach_uses_new_destination_without_tap(cx: &mut TestAppContext) {
    for selected in [false, true] {
        let host = crate::core_mock::Host::default();
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (backend, old_wire) = host.recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, cx| {
            this.select_host("private-core");
            this.connect_core_using(cx, move |_| Ok(backend));
        });
        wait_frame(&view, cx, |this| this.terminal.live.connected);
        let old = cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                prepare(this, window, cx);
                let old = this
                    .terminal
                    .input
                    .destination
                    .clone()
                    .unwrap_or_else(|| panic!("destination"));
                this.terminal.input.fresh_gesture();
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
                if selected {
                    select(this, true, window, cx);
                }
                this.detach_core(window, cx);
                assert!(!this.handoff.select_mode);
                assert_eq!(
                    this.terminal_status(),
                    "Detached · tap Reconnect to reattach"
                );
                key(this, "ctrl-b", false, window, cx);
                this.replace_text_in_range(None, "discarded", window, cx);
                old
            })
        });
        let (backend, wire) = host.recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, cx| {
            this.terminal.core_test_backend = Some(backend);
            this.connect_terminal(cx);
        });
        wait_frame(&view, cx, |this| this.terminal.live.connected);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let _ = this.render_terminal(window, cx);
                assert!(this.terminal_focus.is_focused(window));
                this.terminal.size = None;
                this.refresh_terminal_input();
                assert_ne!(this.terminal.input.destination.as_ref(), Some(&old));
                // Exercise the P9 worker's fence too, bypassing the already-tested UI queue.
                let session = this
                    .terminal
                    .core
                    .as_ref()
                    .unwrap_or_else(|| panic!("session"));
                session
                    .send(crate::core::Command::Input(
                        old,
                        ClientPaneInputEvent::TextCommit("stale".into()),
                    ))
                    .unwrap_or_else(|e| panic!("send: {e}"));
                key(this, "ctrl-a", false, window, cx);
                this.replace_text_in_range(None, "discarded", window, cx);
                key(this, "ctrl-e", false, window, cx); // FIFO end sentinel: byte 5.
            })
        });
        let mut bytes = Vec::new();
        while !bytes.ends_with(&[5]) {
            let (channel, part) = wire
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("wire: {e}"));
            assert_eq!(channel, 1);
            bytes.extend(part);
        }
        assert_eq!(
            bytes,
            [1, 5],
            "no stale destination, discarded text or replay"
        );
        assert!(old_wire.try_iter().next().is_none());
        view.update(cx, |this, _| {
            this.terminal.core.take();
        });
    }
}

#[gpui::test]
fn sh1_156_status_and_highlight_variant_follow_input_gate(cx: &mut TestAppContext) {
    for keep in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
        view.update(cx, |this, _| install(this, client));
        wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                prepare(this, window, cx);
                this.handoff.keep_highlight_on_key = keep;
                select(this, true, window, cx);
                assert!(this.terminal_status().starts_with("Selection active"));
                key(this, "ctrl-a", false, window, cx);
                assert_eq!(this.handoff.selection.is_some(), keep);
                assert!(!this.handoff.blocks_input());
                assert!(this.terminal_status().starts_with("Text input paused"));
                // Marked/commit callbacks remain fenced even with a passive highlight.
                this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
                this.replace_text_in_range(None, "discarded", window, cx);
                assert!(this.terminal.input.composition.text.is_empty());
                if keep {
                    this.copy_selection(cx);
                    assert_eq!(this.handoff.clipboard_writes, 1);
                }
                // A fresh healthy fixture proves the status clears when no refusal remains.
                this.terminal.input = crate::terminal_input::TerminalInput::default();
                this.refresh_terminal_input();
                this.terminal.live.status = "Connected".into();
                assert_eq!(this.terminal_status(), "Connected");
                this.actions.palette = true;
                assert!(this.terminal_status().starts_with("Commands open"));
                this.actions.palette = false;
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
fn sh1_156_terminal_key_keeps_existing_paste_fifo(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, client));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("ordered-paste".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            prepare(this, window, cx);
            this.start_paste(false, cx);
            assert!(this.handoff.preparing);
            key(this, "ctrl-a", false, window, cx);
            assert!(this.handoff.preparing, "key cancelled the reserved paste");
        })
    });
    wait_frame(&view, cx, |this| !this.handoff.preparing);
    view.update(cx, |this, _| sentinel(this, "w1:p1"));
    assert_eq!(
        receive(&wire, "w1:p1"),
        vec![
            ClientPaneInputEvent::Paste("ordered-paste".into()),
            expected_key(ClientKeyCode::Char('a'), 2, ClientKeyKind::Press, false),
        ]
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}
