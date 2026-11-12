use crate::{app::Smoke, input::Input};
use gpui::{EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext};

#[gpui::test]
fn modal_rejects_native_commits_composition_actions_and_clipboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.open = true;
            this.terminal_focus.focus(window, cx);
            this.terminal.input.composition = Input {
                text: "old".into(),
                marked: Some(0..3),
                selected: 3..3,
            };
            this.begin_settings_modal(window, cx);
            assert!(this.settings_modal);
            assert!(this.terminal.input.composition.text.is_empty());
            assert!(!this.accepts_text_input(window, cx));
            let mut actual = Some(0..1);
            assert!(this.text_for_range(0..1, &mut actual, window, cx).is_none());
            assert!(actual.is_none());
            assert!(this.selected_text_range(false, window, cx).is_none());
            assert!(this.marked_text_range(window, cx).is_none());
            assert!(this.text_length_utf16(window, cx).is_none());
            this.set_selected_text_range(0..99, window, cx);
            this.unmark_text(window, cx);
            this.replace_text_in_range(None, "must not send", window, cx);
            this.replace_and_mark_text_in_range(None, "candidate", Some(0..1), window, cx);
            this.start_paste(false, cx);
            this.copy_selection(cx);
            assert_eq!(this.handoff.clipboard_reads, 0);
            assert_eq!(this.handoff.clipboard_writes, 0);
            assert!(
                this.run_action(sheprd_commands::Command::ToggleSidebar)
                    .is_err()
            );
            assert!(this.terminal.input.composition.text.is_empty());
            assert!(this.terminal.input.next().is_none());
            // A disappeared connection must restore the safe setup focus, not old terminal focus.
            this.terminal.open = false;
            this.end_settings_modal(window, cx);
            assert!(!this.settings_modal);
            assert!(this.focus.is_focused(window));
        });
    });
}

#[gpui::test]
fn modal_native_input_has_zero_wire_events_between_positive_controls(cx: &mut TestAppContext) {
    use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
    use std::time::{Duration, Instant};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("{e}"));
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
            this.replace_text_in_range(None, "before", window, cx);
            this.begin_settings_modal(window, cx);
            // Keep terminal focus deliberately: the modal flag must fence late UIKit callbacks.
            this.terminal_focus.focus(window, cx);
            this.replace_text_in_range(None, "leak", window, cx);
            this.replace_and_mark_text_in_range(None, "漏れ", Some(0..1), window, cx);
            this.start_paste(false, cx);
            this.flush_terminal();
            let closing_key = Keystroke {
                key: "enter".into(),
                key_char: None,
                modifiers: Default::default(),
            };
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: closing_key.clone(),
                    is_held: false,
                },
                window,
                cx,
            );
            this.end_settings_modal(window, cx);
            // Neither held closing keys nor late composition can become a fresh gesture.
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: closing_key.clone(),
                    is_held: true,
                },
                window,
                cx,
            );
            this.replace_text_in_range(None, "late-close-tail", window, cx);
            this.terminal_key_up(
                &KeyUpEvent {
                    keystroke: closing_key,
                },
                window,
                cx,
            );
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "after", window, cx);
            this.flush_terminal();
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
        })
    });
    let mut observed = Vec::new();
    'wire: loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire {e}"));
        if let ClientMessage::ClientShellPaneInput { pane_id, events } = message {
            assert_eq!(pane_id, "w1:p1");
            for event in events {
                if event == ClientPaneInputEvent::Paste("barrier".into()) {
                    break 'wire;
                }
                observed.push(event);
            }
        }
    }
    assert_eq!(
        observed,
        vec![
            ClientPaneInputEvent::TextCommit("before".into()),
            ClientPaneInputEvent::TextCommit("after".into())
        ]
    );
}
