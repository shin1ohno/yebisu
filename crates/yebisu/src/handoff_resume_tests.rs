//! Selection exit is an explicit new input gesture; callbacks while selected stay fenced.
use crate::{app::Smoke, mock_peer};
use gpui::{ClipboardItem, EntityInputHandler, MouseButton, TestAppContext, point, px, size};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::time::{Duration, Instant};

#[gpui::test]
fn selection_exit_resumes_soft_text_and_ime_after_paste(cx: &mut TestAppContext) {
    for paste in [true, false] {
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
                if paste {
                    this.start_paste(false, cx);
                    assert!(
                        this.handoff.preparing,
                        "test must exercise queued input during preparation"
                    );
                } else {
                    this.toggle_selection();
                }
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
        eprintln!("D4 PASS paste={paste}: soft text / IME / FIFO / selected late commit rejected");
    }
}
