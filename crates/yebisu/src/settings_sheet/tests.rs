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
                    prefer_character_input: false,
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
                    prefer_character_input: false,
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

#[gpui::test]
fn f19_pending_paste_is_cancelled_before_modal_receiving_barrier(cx: &mut TestAppContext) {
    use gpui::ClipboardItem;
    use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
    use std::time::{Duration, Instant};
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("{e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let g = this.terminal.link.reset();
        this.terminal.link.install(g, Ok(client));
        this.terminal.open = true;
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.write_to_clipboard(ClipboardItem::new_string("P11_LATE_PASTE".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.start_paste(false, cx);
            assert!(this.handoff.preparing);
            this.begin_settings_modal(window, cx);
        })
    });
    loop {
        cx.run_until_parked();
        if view.read_with(cx, |this, _| this.handoff.workers == 0) {
            break;
        }
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
                [ClientPaneInputEvent::Paste("modal-barrier".into())],
            )
            .unwrap_or_else(|e| panic!("{e}"));
    });
    loop {
        let message = receipts
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("{e}"));
        if let ClientMessage::ClientShellPaneInput { events, .. } = message {
            assert_eq!(
                events,
                vec![ClientPaneInputEvent::Paste("modal-barrier".into())]
            );
            break;
        }
    }
}

#[gpui::test]
fn f22_current_destination_rechecked_after_host_generation_boot_and_pane_change(
    cx: &mut TestAppContext,
) {
    use sheprd_client::protocol::ClientPaneInputEvent;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    for change in ["host", "generation", "boot", "pane"] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (client, receipts) =
            crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("{e}"));
        view.update(cx, |this, _| {
            this.select_host("sandbox");
            let g = this.terminal.link.reset();
            this.terminal.link.install(g, Ok(client));
            this.terminal.open = true;
        });
        let until = Instant::now() + Duration::from_secs(5);
        while !view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some()
        }) {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        let barrier_handle = view.read_with(cx, |this, _| {
            this.terminal
                .link
                .handle
                .clone()
                .unwrap_or_else(|| panic!("handle"))
        });
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("F22_LATE_PASTE".into()));
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal.size = None;
                this.terminal_focus.focus(window, cx);
                this.refresh_terminal_input();
                let old = this
                    .terminal
                    .input
                    .destination
                    .clone()
                    .unwrap_or_else(|| panic!("destination"));
                this.start_paste(false, cx);
                assert!(this.handoff.preparing);
                this.begin_settings_modal(window, cx);
                match change {
                    "host" => this.select_host("other-host"),
                    "generation" => {
                        // Keep the receiving stream alive while invalidating the
                        // connection identity observed by every queued callback.
                        this.terminal.link.generation += 1;
                    }
                    _ => {
                        let mut live = this
                            .terminal
                            .link
                            .inbox
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        if let Some(snapshot) = live.snapshot.as_mut() {
                            let snapshot = Arc::make_mut(snapshot);
                            if change == "boot" {
                                snapshot.boot_id = "new-boot".into();
                            } else {
                                snapshot.focused_pane_id = Some("w1:p2".into());
                            }
                        }
                        if change == "boot"
                            && let Some(surface) = live.surface.as_mut()
                        {
                            Arc::make_mut(surface).boot_id = "new-boot".into();
                        }
                        live.input_epoch += 1;
                    }
                }
                // Exercise every actual callback immediately after each change,
                // while the modal is still active (not a combined final state).
                let mut range = Some(0..1);
                assert!(!this.accepts_text_input(window, cx));
                assert!(this.text_for_range(0..1, &mut range, window, cx).is_none());
                assert!(this.selected_text_range(false, window, cx).is_none());
                assert!(this.marked_text_range(window, cx).is_none());
                assert!(this.text_length_utf16(window, cx).is_none());
                let point = gpui::point(gpui::px(0.), gpui::px(0.));
                let bounds = gpui::Bounds::new(point, gpui::size(gpui::px(100.), gpui::px(100.)));
                assert!(this.bounds_for_range(0..1, bounds, window, cx).is_none());
                assert!(this.character_index_for_point(point, window, cx).is_none());
                this.set_selected_text_range(0..1, window, cx);
                this.replace_and_mark_text_in_range(None, "stale mark", Some(0..1), window, cx);
                this.unmark_text(window, cx);
                this.replace_text_in_range(None, "stale commit", window, cx);
                this.start_paste(false, cx);
                this.copy_selection(cx);
                assert!(
                    this.run_action(sheprd_commands::Command::ToggleSidebar)
                        .is_err()
                );
                let key = Keystroke {
                    key: "enter".into(),
                    key_char: None,
                    modifiers: Default::default(),
                };
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: key.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                this.terminal_key_up(
                    &KeyUpEvent {
                        keystroke: key.clone(),
                    },
                    window,
                    cx,
                );
                this.end_settings_modal(window, cx);
                this.replace_and_mark_text_in_range(None, "late mark", Some(0..1), window, cx);
                this.unmark_text(window, cx);
                this.replace_text_in_range(None, "late commit", window, cx);
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: key.clone(),
                        is_held: true,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                this.terminal_key_up(&KeyUpEvent { keystroke: key }, window, cx);
                assert_eq!(this.handoff.clipboard_reads, 1);
                assert_eq!(this.handoff.clipboard_writes, 0);
                assert!(this.terminal.input.composition.text.is_empty());
                assert_ne!(this.terminal.input.destination.as_ref(), Some(&old));
                assert!(
                    !this
                        .terminal
                        .input
                        .queue(crate::terminal_input::GeneratedInput {
                            destination: old,
                            event: ClientPaneInputEvent::TextCommit("stale".into())
                        })
                );
                if change == "host" {
                    assert!(this.focus.is_focused(window));
                } else {
                    assert!(this.terminal_focus.is_focused(window));
                }
                if change != "host" {
                    this.terminal.size = None;
                    this.refresh_terminal_input();
                    this.terminal.input.fresh_gesture();
                    this.replace_text_in_range(None, "fresh", window, cx);
                }
            })
        });
        while view.read_with(cx, |this, _| this.handoff.workers != 0) {
            cx.run_until_parked();
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        barrier_handle
            .send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::Paste("f22-barrier".into())],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        let mut events = Vec::new();
        'receive: loop {
            if let sheprd_client::protocol::ClientMessage::ClientShellPaneInput {
                pane_id,
                events: batch,
                ..
            } = receipts
                .recv_timeout(Duration::from_secs(3))
                .unwrap_or_else(|e| panic!("{e}"))
            {
                for event in batch {
                    if event == ClientPaneInputEvent::Paste("f22-barrier".into()) {
                        assert_eq!(pane_id, "w1:p1");
                        break 'receive;
                    }
                    assert_eq!(pane_id, if change == "pane" { "w1:p2" } else { "w1:p1" });
                    events.push(event);
                }
            }
        }
        assert_eq!(
            events,
            if change == "host" {
                vec![]
            } else {
                vec![ClientPaneInputEvent::TextCommit("fresh".into())]
            },
            "{change}"
        );
        if change == "host" {
            let (client, new_wire) =
                crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("{e}"));
            view.update(cx, |this, _| {
                let generation = this.terminal.link.reset();
                this.terminal.link.install(generation, Ok(client));
            });
            while !view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.surface.is_some()
            }) {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            cx.update(|window, cx| {
                view.update(cx, |this, cx| {
                    this.terminal.size = None;
                    this.terminal_focus.focus(window, cx);
                    this.refresh_terminal_input();
                    this.terminal.input.fresh_gesture();
                    this.replace_text_in_range(None, "new-host-fresh", window, cx);
                    this.terminal
                        .link
                        .handle
                        .as_ref()
                        .unwrap_or_else(|| panic!("handle"))
                        .send_input(
                            "boot-v1",
                            "w1:p1",
                            [ClientPaneInputEvent::Paste("new-host-barrier".into())],
                        )
                        .unwrap_or_else(|e| panic!("{e}"));
                })
            });
            let mut delivered = Vec::new();
            'new_receive: loop {
                if let sheprd_client::protocol::ClientMessage::ClientShellPaneInput {
                    pane_id,
                    events,
                    ..
                } = new_wire
                    .recv_timeout(Duration::from_secs(3))
                    .unwrap_or_else(|e| panic!("{e}"))
                {
                    assert_eq!(pane_id, "w1:p1");
                    for event in events {
                        if event == ClientPaneInputEvent::Paste("new-host-barrier".into()) {
                            break 'new_receive;
                        }
                        delivered.push(event);
                    }
                }
            }
            assert_eq!(
                delivered,
                [ClientPaneInputEvent::TextCommit("new-host-fresh".into())]
            );
        }
    }
}
