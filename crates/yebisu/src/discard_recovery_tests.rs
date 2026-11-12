//! SH1-156 real event adapters; receipts end at a FIFO sentinel, not a sleep.
//! These fixtures prove app routing, not physical UIKit callback delivery.
use crate::{app::Smoke, live::Live, mock_peer};
use gpui::{
    Context, Entity, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext,
    Window,
};
use sheprd_client::protocol::{ClientKeyCode, ClientKeyKind, ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub(crate) fn wait_frame(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    ready: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            ready(this)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "SH1-156 frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn key(
    this: &mut Smoke,
    name: &str,
    held: bool,
    window: &mut Window,
    cx: &mut Context<Smoke>,
) {
    this.terminal_key_down(
        &KeyDownEvent {
            keystroke: Keystroke::parse(name).unwrap_or_else(|e| panic!("key: {e}")),
            is_held: held,
            prefer_character_input: false,
        },
        window,
        cx,
    );
}
pub(crate) fn release(this: &mut Smoke, name: &str, window: &mut Window, cx: &mut Context<Smoke>) {
    this.terminal_key_up(
        &KeyUpEvent {
            keystroke: Keystroke::parse(name).unwrap_or_else(|e| panic!("key: {e}")),
        },
        window,
        cx,
    );
}
pub(crate) fn install(this: &mut Smoke, client: sheprd_client::Client) {
    this.select_host("sandbox");
    let generation = this.terminal.link.reset();
    this.terminal.link.install(generation, Ok(client));
    this.terminal.open = true;
}
pub(crate) fn prepare(this: &mut Smoke, window: &mut Window, cx: &mut Context<Smoke>) {
    this.terminal_focus.focus(window, cx);
    this.terminal.size = None;
    this.refresh_terminal_input();
    this.sync_handoff();
}
pub(crate) fn select(this: &mut Smoke, range: bool, window: &mut Window, cx: &mut Context<Smoke>) {
    this.toggle_selection();
    if range {
        this.terminal.geometry = crate::input_geometry::InputGeometry {
            grid: gpui::Bounds::new(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(gpui::px(640.), gpui::px(480.)),
            ),
            cell_width: 8.,
            cell_height: 20.,
        };
        this.selection_down(
            &gpui::MouseDownEvent {
                position: gpui::point(gpui::px(12.), gpui::px(30.)),
                button: gpui::MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
                first_mouse: false,
            },
            window,
            cx,
        );
        assert!(
            this.handoff.selection.is_some(),
            "real selection adapter must select a cell"
        );
    }
}
pub(crate) fn sentinel(this: &mut Smoke, pane: &str) {
    this.terminal.size = None;
    this.refresh_terminal_input();
    this.flush_terminal();
    assert!(
        this.terminal.input.ready,
        "drain size-held inputs before sentinel"
    );
    this.terminal
        .link
        .handle
        .as_ref()
        .unwrap_or_else(|| panic!("handle"))
        .send_input(
            "boot-v1",
            pane,
            [ClientPaneInputEvent::Paste("SH1-156-end".into())],
        )
        .unwrap_or_else(|e| panic!("sentinel: {e}"));
}
pub(crate) fn receive(
    wire: &mpsc::Receiver<ClientMessage>,
    pane: &str,
) -> Vec<ClientPaneInputEvent> {
    let mut result = Vec::new();
    loop {
        let message = wire
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("wire: {e}"));
        match message {
            ClientMessage::ClientShellPaneInput { pane_id, events } => {
                assert_eq!(pane_id, pane, "no input to a stale target");
                for event in events {
                    if event == ClientPaneInputEvent::Paste("SH1-156-end".into()) {
                        return result;
                    }
                    result.push(event);
                }
            }
            ClientMessage::ClientShellPopupInput { .. } => panic!("input escaped to popup"),
            _ => {}
        }
    }
}
pub(crate) fn expected_key(
    code: ClientKeyCode,
    modifiers: u8,
    kind: ClientKeyKind,
    report: bool,
) -> ClientPaneInputEvent {
    ClientPaneInputEvent::Key {
        code,
        modifiers,
        kind,
        repeat_count: 1,
        shifted_codepoint: None,
        generated_text: None,
        tracks_release: report,
        physical_key_id: None,
        windows_record: None,
    }
}

#[gpui::test]
fn sh1_156_selection_new_press_resumes_without_discarded_commit(cx: &mut TestAppContext) {
    for report in [false, true] {
        for range in [false, true] {
            let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
            let (client, wire) =
                mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
            view.update(cx, |this, _| install(this, client));
            wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
            cx.update(|window, cx| {
                view.update(cx, |this, cx| {
                    prepare(this, window, cx);
                    this.terminal
                        .link
                        .inbox
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .keyboard_report_all = report;
                    this.terminal.input.fresh_gesture();
                    this.replace_and_mark_text_in_range(None, "discarded", None, window, cx);
                    select(this, range, window, cx);
                    this.replace_text_in_range(None, "discarded", window, cx);
                    // A repeat from an earlier press must neither exit selection nor leak.
                    key(this, "ctrl-a", true, window, cx);
                    assert!(this.handoff.select_mode, "repeat exited selection");
                    if range {
                        key(this, "cmd-c", false, window, cx);
                        release(this, "cmd-c", window, cx);
                        assert!(this.handoff.select_mode, "Copy exited selection");
                        assert_eq!(this.handoff.clipboard_writes, 1);
                    }
                    key(this, "ctrl-a", false, window, cx);
                    assert!(!this.handoff.select_mode, "fresh key retained Select mode");
                    assert!(this.handoff.selection.is_none(), "default clears highlight");
                    this.replace_text_in_range(None, "discarded", window, cx);
                    release(this, "ctrl-a", window, cx);
                    key(this, "left", false, window, cx);
                    release(this, "left", window, cx);
                    assert!(
                        this.terminal.input.requires_gesture(),
                        "key retired no native IME session"
                    );
                    sentinel(this, "w1:p1");
                })
            });
            let mut expected = Vec::new();
            for (code, modifiers) in [(ClientKeyCode::Char('a'), 2), (ClientKeyCode::Left, 0)] {
                expected.push(expected_key(
                    code.clone(),
                    modifiers,
                    ClientKeyKind::Press,
                    report,
                ));
                if report {
                    expected.push(expected_key(
                        code,
                        modifiers,
                        ClientKeyKind::Release,
                        report,
                    ));
                }
            }
            assert_eq!(
                receive(&wire, "w1:p1"),
                expected,
                "selection range={range}, report={report}"
            );
            view.update(cx, |this, _| this.terminal.link.pause());
        }
    }
}

#[gpui::test]
fn sh1_156_generation_change_closes_select_mode_before_new_input(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (old, old_wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, old));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    let old_destination = cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            prepare(this, window, cx);
            let destination = this
                .terminal
                .input
                .destination
                .clone()
                .unwrap_or_else(|| panic!("destination"));
            select(this, true, window, cx);
            this.terminal.link.reset();
            this.terminal.live = Live::default();
            this.refresh_terminal_input();
            this.sync_handoff();
            assert!(
                !this.handoff.select_mode,
                "reconnect carried selection mode"
            );
            key(this, "ctrl-b", false, window, cx);
            destination
        })
    });
    let (new, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.terminal
            .link
            .install(this.terminal.link.generation, Ok(new));
    });
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            this.refresh_terminal_input();
            assert_ne!(
                this.terminal.input.destination.as_ref(),
                Some(&old_destination)
            );
            this.send_terminal(crate::terminal_input::GeneratedInput {
                destination: old_destination,
                event: ClientPaneInputEvent::TextCommit("stale-generation".into()),
            });
            key(this, "ctrl-a", false, window, cx);
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
        ClientMessage::ClientShellPaneInput { .. } | ClientMessage::ClientShellPopupInput { .. }
    )));
    view.update(cx, |this, _| this.terminal.link.pause());
}

#[gpui::test]
fn sh1_156_response_before_projection_keeps_real_key_adapter_fenced(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, wire, release_projection) =
        mock_peer::connect_response_first().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| install(this, client));
    wait_frame(&view, cx, |this| this.terminal.live.surface.is_some());
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            prepare(this, window, cx);
            this.navigate_board(
                "sandbox",
                crate::board::Target::Agent("w2:p2".into()),
                "boot-v1",
                this.terminal.link.generation,
            )
            .unwrap_or_else(|e| panic!("navigate: {e}"));
            key(this, "ctrl-b", false, window, cx);
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
            key(this, "ctrl-c", false, window, cx);
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
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.size = None;
            key(this, "ctrl-a", false, window, cx);
            key(this, "left", false, window, cx);
            sentinel(this, "w2:p2");
        })
    });
    assert_eq!(
        receive(&wire, "w2:p2"),
        vec![
            expected_key(ClientKeyCode::Char('a'), 2, ClientKeyKind::Press, false),
            expected_key(ClientKeyCode::Left, 0, ClientKeyKind::Press, false),
        ]
    );
    view.update(cx, |this, _| this.terminal.link.pause());
}
