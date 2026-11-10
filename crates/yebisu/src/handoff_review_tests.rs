//! Real GPUI event adapters and private-peer receipts for P8 review D1/D2.
use crate::{app::Smoke, handoff::Selection, mock_peer};
use gpui::{
    ClipboardItem, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, TestAppContext,
    point, px, size,
};
use sheprd_client::{
    ClientEvent,
    protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[gpui::test]
fn clipboard_shortcuts_honor_all_command_priority_combinations(cx: &mut TestAppContext) {
    for command_control in [false, true] {
        for terminal_priority in [false, true] {
            for key in ["c", "v"] {
                let terminal_route = command_control && terminal_priority;
                let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
                let (client, receipts) =
                    mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
                view.update(cx, |this, _| {
                    this.select_host("sandbox");
                    let generation = this.terminal.link.reset();
                    this.terminal.link.install(generation, Ok(client));
                });
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    if view.update(cx, |this, cx| {
                        this.poll_terminal(cx);
                        this.terminal.live.surface.is_some()
                    }) {
                        break;
                    }
                    assert!(Instant::now() < deadline, "reader timeout");
                    std::thread::sleep(Duration::from_millis(5));
                }
                cx.write_to_clipboard(ClipboardItem::new_string("clipboard-sentinel".into()));
                cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        this.terminal_focus.focus(window, cx);
                        this.terminal.input.options.command_control = command_control;
                        this.actions.terminal_priority = terminal_priority;
                        this.terminal
                            .link
                            .inbox
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .keyboard_report_all = true;
                        this.terminal.size = None;
                        this.refresh_terminal_input();
                        this.sync_handoff();
                        if !terminal_route && key == "c" {
                            let frame = this
                                .terminal
                                .live
                                .surface
                                .as_ref()
                                .unwrap_or_else(|| panic!("surface"))
                                .frame
                                .clone();
                            this.handoff.selection = Some(Selection {
                                frame: Arc::new(frame),
                                anchor: (1, 2),
                                end: (6, 2),
                                dragging: false,
                            });
                        }
                        let keystroke = Keystroke {
                            key: key.into(),
                            key_char: None,
                            modifiers: Modifiers {
                                platform: true,
                                ..Default::default()
                            },
                        };
                        this.terminal_key_down(
                            &KeyDownEvent {
                                keystroke: keystroke.clone(),
                                is_held: false,
                                prefer_character_input: false,
                            },
                            window,
                            cx,
                        );
                        this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
                    })
                });
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    cx.run_until_parked();
                    if view.update(cx, |this, _| !this.handoff.preparing) {
                        break;
                    }
                    assert!(Instant::now() < deadline, "paste timeout");
                }
                view.update(cx, |this, _| {
                    assert_eq!(
                        this.handoff.clipboard_reads,
                        usize::from(!terminal_route && key == "v"),
                        "read: Ctrl={command_control} priority={terminal_priority} key={key}"
                    );
                    assert_eq!(
                        this.handoff.clipboard_writes,
                        usize::from(!terminal_route && key == "c"),
                        "write: Ctrl={command_control} priority={terminal_priority} key={key}"
                    );
                    // A wire barrier proves the no-extra-event assertions after both key edges.
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
                        .unwrap_or_else(|e| panic!("barrier: {e}"));
                });
                assert_eq!(
                    cx.read_from_clipboard()
                        .and_then(|item| item.text())
                        .as_deref(),
                    Some(if !terminal_route && key == "c" {
                        "日本語"
                    } else {
                        "clipboard-sentinel"
                    })
                );
                let mut events = Vec::new();
                let deadline = Instant::now() + Duration::from_secs(5);
                'wire: loop {
                    let message = receipts
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .unwrap_or_else(|e| panic!("wire: {e}"));
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
                if terminal_route {
                    assert_eq!(events.len(), 2, "Ctrl-{key} press and release");
                    for (event, expected_kind) in events
                        .iter()
                        .zip([ClientKeyKind::Press, ClientKeyKind::Release])
                    {
                        let ClientPaneInputEvent::Key {
                            code,
                            modifiers,
                            kind,
                            ..
                        } = event
                        else {
                            panic!("not semantic Ctrl key: {event:?}")
                        };
                        assert_eq!(
                            *code,
                            ClientKeyCode::Char(if key == "c" { 'c' } else { 'v' })
                        );
                        assert_eq!(*modifiers, 2); // protocol Ctrl bit (terminal.rs)
                        assert_eq!(*kind, expected_kind);
                    }
                } else {
                    assert_eq!(
                        events,
                        if key == "v" {
                            vec![ClientPaneInputEvent::Paste("clipboard-sentinel".into())]
                        } else {
                            vec![]
                        }
                    );
                }
                view.update(cx, |this, _| {
                    this.terminal.link.pause();
                });
                eprintln!(
                    "D1 PASS Ctrl={command_control} terminal_priority={terminal_priority} key={key}"
                );
            }
        }
    }
}

#[gpui::test]
fn outside_release_ends_drag_and_copies_once_before_buttonless_move(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        this.terminal.live.apply(ClientEvent::Snapshot(Arc::new(
            serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}")),
        )));
        this.terminal
            .live
            .apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
        this.terminal.live.connected = true;
        this.terminal.link.inbox = Arc::new(std::sync::Mutex::new(this.terminal.live.clone()));
        this.handoff.select_mode = true;
        this.handoff.copy_on_select = true;
        cx.notify();
    });
    cx.run_until_parked();
    let g = view.update(cx, |this, _| this.terminal.geometry);
    assert!(
        g.grid.size.height > px(60.),
        "real terminal must be laid out"
    );
    let at =
        |col: f32| g.grid.origin + point(px((col + 0.5) * g.cell_width), px(2.5 * g.cell_height));
    cx.write_to_clipboard(ClipboardItem::new_string("untouched".into()));
    cx.simulate_mouse_move(at(1.), None, Modifiers::default());
    cx.simulate_mouse_down(at(1.), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(at(6.), Some(MouseButton::Left), Modifiers::default());
    view.update(cx, |this, _| {
        assert!(this.handoff.selection.as_ref().is_some_and(|s| s.dragging))
    });
    let outside = point(px(2.), px(2.));
    assert!(!g.contains(outside));
    cx.simulate_mouse_move(outside, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
    let end = view.update(cx, |this, _| {
        let s = this
            .handoff
            .selection
            .as_ref()
            .unwrap_or_else(|| panic!("selection"));
        assert!(!s.dragging, "release outside must end drag");
        assert_eq!(this.handoff.clipboard_writes, 1);
        s.end
    });
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("日本語")
    );
    cx.simulate_mouse_move(at(12.), None, Modifiers::default());
    view.update(cx, |this, _| {
        assert_eq!(this.handoff.selection.as_ref().map(|s| s.end), Some(end));
        assert_eq!(this.handoff.clipboard_writes, 1);
        // Independently test the guard when a release was lost by the platform.
        if let Some(s) = &mut this.handoff.selection {
            s.dragging = true;
        }
    });
    cx.simulate_mouse_move(at(14.), None, Modifiers::default());
    view.update(cx, |this, _| {
        assert_eq!(
            this.handoff.selection.as_ref().map(|s| s.end),
            Some(end),
            "no-button move cannot extend even a stale drag"
        )
    });
}
