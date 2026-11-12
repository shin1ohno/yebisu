//! D1-D4 regressions exercise production callbacks and actual receiving sockets.
#![allow(clippy::unwrap_used)]
use super::available;
use crate::{
    app::Smoke,
    find::{State, field::Field},
};
use gpui::{
    ClipboardItem, Entity, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext,
    VisualTestContext,
};
use sheprd_client::{
    Client,
    protocol::{ClientKeyKind, ClientMessage, ClientPaneInputEvent},
};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};
fn key(name: &str, held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap(),
        is_held: held,
        prefer_character_input: false,
    }
}
fn install(view: &Entity<Smoke>, cx: &mut VisualTestContext, host: &str, client: Client) {
    view.update(cx, |this, _| {
        this.select_host(host);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            available(&this.terminal)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "P13 peer startup");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn open(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Entity<Field> {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_find(window, cx);
            this.find.as_ref().unwrap().field.clone()
        })
    })
}
fn edit(field: &Entity<Field>, cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.input.selected = 0..field.input.text.encode_utf16().count();
            field.replace_text_in_range(None, text, window, cx);
        })
    });
}
fn matched(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.poll_find(cx);
            this.find
                .as_ref()
                .is_some_and(|b| b.search.state == State::Matches)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "P13 actual socket result");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn terminal(receipts: &Receiver<ClientMessage>) -> Vec<ClientPaneInputEvent> {
    receipts
        .try_iter()
        .flat_map(|m| match m {
            ClientMessage::ClientShellPaneInput { events, .. } => events,
            _ => Vec::new(),
        })
        .collect()
}
#[gpui::test]
fn d2_close_escape_owns_repeat_and_release_in_both_report_modes(cx: &mut TestAppContext) {
    for report_all in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let (client, receipts) = crate::find::peer::connect().unwrap();
        install(&view, cx, "d2", client);
        view.update(cx, |this, _| {
            this.terminal.link.inbox.lock().unwrap().keyboard_report_all = report_all;
        });
        let field = open(&view, cx);
        cx.update(|window, cx| {
            field.update(cx, |field, cx| field.key(&key("escape", false), window, cx))
        });
        view.update(cx, |this, _| {
            assert!(this.find.is_none());
            assert!(
                this.actions.consumed.contains("escape"),
                "D2 closing Escape lost press ownership"
            );
        });
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal.size = None;
                this.replace_text_in_range(None, "late native commit", window, cx);
                this.terminal_key_down(&key("escape", true), window, cx);
                this.terminal_key_down(&key("escape", true), window, cx);
                this.terminal_key_up(
                    &KeyUpEvent {
                        keystroke: key("escape", false).keystroke,
                    },
                    window,
                    cx,
                );
            })
        });
        std::thread::sleep(Duration::from_millis(60));
        assert!(
            terminal(&receipts).is_empty(),
            "D2 held/release/native callback leaked terminal bytes"
        );
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_key_down(&key("escape", false), window, cx);
                this.terminal_key_up(
                    &KeyUpEvent {
                        keystroke: key("escape", false).keystroke,
                    },
                    window,
                    cx,
                );
            })
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut events = Vec::new();
        while events.len() < if report_all { 2 } else { 1 } {
            let message = receipts
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if let ClientMessage::ClientShellPaneInput {
                pane_id,
                events: next,
            } = message
            {
                assert_eq!(pane_id, "w1:p1");
                events.extend(next);
            }
        }
        assert!(
            matches!(
                &events[0],
                ClientPaneInputEvent::Key {
                    kind: ClientKeyKind::Press,
                    ..
                }
            ),
            "D2 fresh press did not reach current pane once"
        );
        if report_all {
            assert!(matches!(
                &events[1],
                ClientPaneInputEvent::Key {
                    kind: ClientKeyKind::Release,
                    ..
                }
            ));
        }
        assert!(!events.iter().any(|e| matches!(
            e,
            ClientPaneInputEvent::Key {
                kind: ClientKeyKind::Repeat,
                ..
            }
        )));
    }
}
#[gpui::test]
fn d3_native_partial_mark_delete_replace_then_escape_keeps_search_open(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, "d3", client);
    let field = open(&view, cx);
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.input.set_text("pre::post".into());
            field.replace_and_mark_text_in_range(Some(4..4), "変😀中", Some(1..3), window, cx);
            assert_eq!(field.input.marked, Some(4..8));
            field.replace_text_in_range(Some(5..7), "", window, cx);
            assert_eq!(field.input.text, "pre:変中:post");
            assert_eq!(
                field.input.marked,
                Some(4..6),
                "D3 native partial deletion committed remaining mark"
            );
            field.replace_text_in_range(Some(5..6), "😀", window, cx);
            assert_eq!(
                field.input.marked,
                Some(4..7),
                "D3 UTF16 replacement lost marked span"
            );
            field.key(&key("escape", false), window, cx);
            assert!(field.input.marked.is_none());
            assert_eq!(field.input.text, "pre::post");
        })
    });
    view.update(cx, |this, _| {
        assert!(
            this.find.is_some(),
            "D3 Escape after partial edit closed search"
        )
    });
    assert!(terminal(&receipts).is_empty());
}
#[gpui::test]
fn d4_native_cut_preserves_unselected_mark_then_escape(cx: &mut TestAppContext) {
    cut_then_escape(cx, false);
}
#[gpui::test]
fn d4_command_x_preserves_unselected_mark_then_escape(cx: &mut TestAppContext) {
    cut_then_escape(cx, true);
}
fn cut_then_escape(cx: &mut TestAppContext, command: bool) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, "d4", client);
    let field = open(&view, cx);
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.input.set_text("pre::post".into());
            field.replace_and_mark_text_in_range(Some(4..4), "変😀中", Some(1..3), window, cx);
            assert_eq!(field.input.text, "pre:変😀中:post");
            assert_eq!(field.input.marked, Some(4..8));
            assert_eq!(field.input.selected, 5..7);
            cx.write_to_clipboard(ClipboardItem::new_string("sentinel"));
            if command {
                field.key(&key("cmd-x", false), window, cx);
            } else {
                // This is the production native Cut callback. The Simulator
                // gate separately exercises UIKit's selector and origin guard.
                field.cut(window, cx);
            }
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("😀"),
                "D4 Cut copied outside the explicit UTF16 selection"
            );
            assert_eq!(
                field.input.text, "pre:変中:post",
                "D4 Cut deleted unselected marked text"
            );
            assert_eq!(
                field.input.marked,
                Some(4..6),
                "D4 Cut committed remaining mark"
            );
            assert_eq!(field.input.selected, 5..5);
            field.key(&key("escape", false), window, cx);
            assert_eq!(field.input.text, "pre::post");
            assert!(field.input.marked.is_none());
        })
    });
    view.update(cx, |this, _| {
        assert!(this.find.is_some(), "D4 Escape after Cut closed search");
    });
    // A later round-trip on the same FIFO socket is a receiving-side barrier.
    edit(&field, cx, "needle");
    matched(&view, cx);
    assert!(
        terminal(&receipts).is_empty(),
        "D4 Cut/Escape leaked terminal bytes"
    );
}
#[gpui::test]
fn d4_ime_commit_none_still_replaces_whole_marked_clause(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, "d4-commit", client);
    let field = open(&view, cx);
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.input.set_text("pre::post".into());
            field.replace_and_mark_text_in_range(Some(4..4), "変😀中", Some(1..3), window, cx);
            field.replace_text_in_range(None, "確定", window, cx);
            assert_eq!(
                field.input.text, "pre:確定:post",
                "D4 IME None contract changed"
            );
            assert!(field.input.marked.is_none());
            assert_eq!(field.input.selected, 6..6);
        })
    });
    edit(&field, cx, "needle");
    matched(&view, cx);
    assert!(terminal(&receipts).is_empty());
}
#[gpui::test]
fn offscreen_match_receives_scroll_and_paints_the_new_viewport(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, "scroll", client);
    let field = open(&view, cx);
    let expected_offset = view.update(cx, |this, _| {
        u64::from(
            this.terminal.live.surface.as_ref().unwrap().panes[0]
                .inner_rect
                .height
                / 2,
        )
    });
    edit(&field, cx, "beyond");
    matched(&view, cx);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            !this.find_highlights().is_empty()
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "offscreen result was never scrolled into painter viewport"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut saw_scroll = false;
    while let Ok(message) = receipts.try_recv() {
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = message {
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            if request["method"] == "pane.scroll" {
                saw_scroll = true;
                assert_eq!(request["params"]["offset_from_bottom"], expected_offset);
            }
        }
    }
    assert!(
        saw_scroll,
        "offscreen search did not reach pane.scroll receiving peer"
    );
    view.update(cx, |this, _| {
        let surface = this.terminal.live.surface.as_ref().unwrap();
        assert_eq!(
            surface.panes[0].scroll.unwrap().offset_from_bottom,
            expected_offset
        );
        assert_eq!(u64::from(this.find_highlights()[0].row), expected_offset);
    });
}
#[gpui::test]
fn delayed_wrong_boot_socket_reply_cannot_adopt_a_colliding_new_connection_id(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, old_receipts, release) = crate::find::peer::connect_delayed(true).unwrap();
    install(&view, cx, "old", client);
    let field = open(&view, cx);
    edit(&field, cx, "needle");
    let request_id = |receipts: &Receiver<ClientMessage>| -> String {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let ClientMessage::ClientShellEndpointRequest { request, .. } = receipts
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap()
            {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                if request["method"] == "pane.copy_search" {
                    return request["id"].as_str().unwrap().to_owned();
                }
            }
        }
    };
    let old_id = request_id(&old_receipts);
    cx.update(|window, cx| view.update(cx, |this, cx| this.close_find(window, cx)));
    let (client, new_receipts) = crate::find::peer::connect().unwrap();
    install(&view, cx, "new", client);
    let fresh = open(&view, cx);
    edit(&fresh, cx, "日本語");
    matched(&view, cx);
    assert_eq!(
        request_id(&new_receipts),
        old_id,
        "controlled connections did not exercise colliding request IDs"
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let disconnected = view.update(cx, |this, cx| {
            this.poll_hidden(cx);
            !this.board.hidden["old"]
                .link
                .inbox
                .lock()
                .unwrap()
                .connected
        });
        if disconnected {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "wrong-boot socket reply did not exercise SDK receiving reject"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        this.poll_find(cx);
        let bar = this.find.as_ref().unwrap();
        assert_eq!(bar.search.scope.host, "new");
        assert_eq!(bar.search.query, "日本語");
        assert_eq!(bar.search.state, State::Matches);
    });
}

#[test]
fn d1_peer_resize_owns_a_monotonic_surface_serial() {
    use sheprd_client::{ClientEvent, ConnectOptions};
    let (client, _receipts) = crate::find::peer::connect().unwrap();
    let surface = |client: &Client| {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let event = client
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| {
                    panic!("D1 resize did not produce an increasing surface revision")
                });
            if let ClientEvent::Surface(frame) = event {
                return frame;
            }
        }
    };
    assert_eq!(surface(&client).surface_revision, 1);
    for (cols, revision) in [(90, 2), (100, 3)] {
        let mut options = ConnectOptions::default();
        options.surface_size.cols = cols;
        client.handle.resize("boot-v1", options).unwrap();
        assert_eq!(
            surface(&client).surface_revision,
            revision,
            "D1 resize serial was not initialized to the installed frame"
        );
    }
}
