//! Production field + reader + Link + controlled receiving socket, not a model.
#![allow(clippy::unwrap_used)]
use super::*;
use gpui::{EntityInputHandler, TestAppContext};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::time::Duration;
fn key(name: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap(),
        is_held: false,
        prefer_character_input: false,
    }
}
#[gpui::test]
fn production_field_searches_utf8_paints_and_restores_one_fresh_terminal_commit(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = crate::find::peer::connect().unwrap();
    view.update(cx, |this, _| {
        this.select_host("p13-peer");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            available(&this.terminal)
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "capable receiving peer never ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let field = cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_find(window, cx);
            this.find.as_ref().unwrap().field.clone()
        })
    });
    for query in ["needle", "日本語"] {
        let deadline = Instant::now() + Duration::from_secs(10);
        cx.update(|window, cx| {
            field.update(cx, |field, cx| {
                field.input.selected = 0..field.input.text.encode_utf16().count();
                field.replace_text_in_range(None, query, window, cx);
            })
        });
        loop {
            if view.update(cx, |this, cx| {
                this.poll_find(cx);
                this.find.as_ref().is_some_and(|bar| {
                    bar.search.state == State::Matches && bar.search.query == query
                })
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "search response not applied");
            std::thread::sleep(Duration::from_millis(5));
        }
        view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            assert!(
                !this.find_highlights().is_empty(),
                "result did not reach production painter adapter"
            );
            assert_eq!(
                this.find
                    .as_ref()
                    .unwrap()
                    .search
                    .result
                    .as_ref()
                    .unwrap()
                    .total,
                2
            );
        });
        let before = view.update(cx, |this, _| {
            this.find
                .as_ref()
                .unwrap()
                .search
                .result
                .as_ref()
                .unwrap()
                .current
        });
        cx.update(|window, cx| field.update(cx, |field, cx| field.key(&key("enter"), window, cx)));
        loop {
            if view.update(cx, |this, cx| {
                this.poll_find(cx);
                this.find
                    .as_ref()
                    .unwrap()
                    .search
                    .result
                    .as_ref()
                    .is_some_and(|r| r.current != before)
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "変換中", Some(0..1), window, cx);
            assert!(field.input.marked.is_some());
            field.key(&key("escape"), window, cx);
            assert!(field.input.marked.is_none());
        })
    });
    view.update(cx, |this, _| {
        assert!(this.find.is_some(), "composing escape closed field")
    });
    // A stale terminal native callback is fenced even if focus was stolen.
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.replace_text_in_range(None, "must-not-send", window, cx);
            this.terminal_key_down(&key("escape"), window, cx);
            this.close_find(window, cx);
            assert!(this.terminal_focus.is_focused(window));
            this.terminal.size = None;
            this.terminal_key_down(&key("f"), window, cx);
            this.replace_text_in_range(None, "fresh-only", window, cx);
        })
    });
    // A retained old native entity cannot accept callbacks after close.
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_text_in_range(None, "retired", window, cx)
        })
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut queries = Vec::new();
    let mut terminal = Vec::new();
    while let Ok(message) =
        receipts.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    {
        match message {
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                if request["method"] == "pane.copy_search" {
                    queries.push(request["params"]["query"].as_str().unwrap().to_owned());
                }
            }
            ClientMessage::ClientShellPaneInput { events, .. } => terminal.extend(events),
            _ => {}
        }
        if terminal.len() == 1 {
            break;
        }
    }
    assert!(queries.iter().any(|q| q == "needle"));
    assert!(queries.iter().any(|q| q == "日本語"));
    assert_eq!(
        terminal,
        vec![ClientPaneInputEvent::TextCommit("fresh-only".into())]
    );
    assert!(
        !receipts
            .try_iter()
            .any(|m| matches!(m, ClientMessage::ClientShellPaneInput { .. }))
    );
    println!(
        "P13 search native adapter: exact ASCII/Japanese queries received; zero terminal bytes until close; one fresh post-close commit; physical IME unverified"
    );
}

#[gpui::test]
fn authoritative_target_and_connection_retire_before_a_colliding_reply_can_paint(
    cx: &mut TestAppContext,
) {
    use sheprd_client::{ClientEvent, protocol::ClientShellSnapshot};
    for fault in [
        "generation",
        "boot",
        "pane",
        "epoch",
        "mailbox",
        "disconnect",
    ] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        view.update(cx, |this, _| {
            this.select_host("a");
            let mut live = Live::default();
            live.connected = true;
            live.methods = vec!["pane.copy_search".into()];
            let snapshot: ClientShellSnapshot =
                serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap();
            live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
            live.apply(ClientEvent::Surface(Arc::new(crate::find::peer::surface())));
            this.terminal.live = live.clone();
            *this.terminal.link.inbox.lock().unwrap() = live;
        });
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.open_find(window, cx);
                let bar = this.find.as_mut().unwrap();
                bar.search.set_query("needle");
                let params = bar.search.next(4, TextPoint::default()).unwrap();
                bar.search.sent("collision".into(), &params, Instant::now());
                let mut live = bar.inbox.lock().unwrap();
                live.search.ticket = Some((bar.search.scope.clone(), "collision".into()));
            })
        });
        view.update(cx,|this,cx| {
            let old=this.terminal.link.inbox.clone();
            {
                let mut live=old.lock().unwrap();
                match fault {
                    "generation"=>this.terminal.link.generation+=1,
                    "boot"=> {
                        let mut snapshot=live.snapshot.as_deref().unwrap().clone();
                        let mut surface=live.surface.as_deref().unwrap().clone();
                        snapshot.boot_id="new-boot".into();surface.boot_id="new-boot".into();
                        live.snapshot=Some(Arc::new(snapshot));live.surface=Some(Arc::new(surface));
                    }
                    "pane"=> {
                        let mut snapshot=live.snapshot.as_deref().unwrap().clone();
                        snapshot.focused_pane_id=Some("other-pane".into());
                        live.snapshot=Some(Arc::new(snapshot));
                    }
                    "epoch"=>live.input_epoch+=1,
                    "mailbox"=>{},
                    "disconnect"=>live.apply(ClientEvent::Disconnected {reason:"after send".into(),ssh:None}),
                    _=>unreachable!(),
                }
                // This is the reader's real response dispatch, before UI focus updates.
                live.apply(ClientEvent::Response {request_id:"collision".into(),response:serde_json::json!({"result":{"type":"pane_copy_search","pane_id":"w1:p1","content_revision":4,"matches":[],"total":0}})});
            }
            if fault=="mailbox" {
                // A new host can reuse the same boot/pane/generation and request ID.
                // The connection mailbox identity is still different.
                this.terminal.link.inbox=Arc::new(Mutex::new(this.terminal.live.clone()));
                this.terminal.reconnect.target="b".into();
            }
            this.poll_find(cx);
            assert!(this.find.is_none(),"{fault} did not retire the production adapter");
            assert!(this.find_highlights().is_empty());
        });
    }
}
