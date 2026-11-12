use super::*;
use crate::{app::Smoke, live::Live};
use gpui::{Entity, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext};
use std::time::{Duration, Instant};

fn wait_frame(view: &Entity<Smoke>, cx: &mut TestAppContext, predicate: impl Fn(&Live) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            predicate(&this.terminal.live)
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "expected core frame was not published: {}",
            view.update(cx, |this, _| this.terminal.live.status.clone())
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn text(live: &Live) -> String {
    live.surface
        .as_ref()
        .map(|s| s.frame.cells.iter().map(|c| c.symbol.as_str()).collect())
        .unwrap_or_default()
}
fn start(view: &Entity<Smoke>, cx: &mut TestAppContext) -> mpsc::Receiver<(u32, Vec<u8>)> {
    let (backend, wire) = crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.select_host("private-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_frame(view, cx, |live| live.connected);
    wire
}
fn command(view: &Entity<Smoke>, cx: &mut TestAppContext, command: Command) {
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core session"))
            .send(command)
            .unwrap_or_else(|e| panic!("command: {e}"));
    });
}

fn receive(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    wire: &mpsc::Receiver<(u32, Vec<u8>)>,
) -> (u32, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // UIKit paints can request another resize. Pump the normal UI poll so
        // its matching frame releases queued input, as the running app does.
        view.update(cx, |this, cx| this.poll_terminal(cx));
        if let Ok(receipt) = wire.recv_timeout(Duration::from_millis(5)) {
            return receipt;
        }
        assert!(
            Instant::now() < deadline,
            "wire timeout: {:?}",
            view.update(cx, |this, _| (
                this.terminal.input.ready,
                this.terminal.size,
                this.terminal.live.status.clone()
            ))
        );
    }
}

#[gpui::test]
fn hidden_palette_shortcut_keeps_core_text_and_paste_live(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let wire = start(&view, cx);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("PASTE".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.refresh_terminal_input();
            let keystroke = Keystroke::parse("cmd-shift-p").unwrap_or_else(|e| panic!("key: {e}"));
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
            assert!(
                !this.actions.palette,
                "core shortcut opened an invisible palette"
            );
            this.replace_text_in_range(None, "TEXT", window, cx);
            this.start_paste(false, cx);
            this.replace_text_in_range(None, "END", window, cx);
        })
    });
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"END") {
        let (channel, part) = receive(&view, cx, &wire);
        assert_eq!(channel, 1);
        bytes.extend(part);
    }
    assert_eq!(bytes, b"TEXTPASTEEND");
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
fn pending_herdr1_navigation_is_discarded_before_first_core_frame(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, old_wire, release) =
        crate::mock_peer::connect_controlled().unwrap_or_else(|e| panic!("old peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("private-core");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait_frame(&view, cx, |live| live.surface.is_some());
    view.update(cx, |this, _| {
        this.run_action(sheprd_commands::Command::Tab)
            .unwrap_or_else(|e| panic!("navigate: {e}"));
    });
    loop {
        let request = old_wire
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("old request: {e}"));
        if matches!(
            request,
            sheprd_client::protocol::ClientMessage::ClientShellEndpointRequest { .. }
        ) {
            break;
        }
    }
    let (backend, wire) = crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let inbox = view.update(cx, |this, cx| {
        assert!(this.terminal.navigating.is_some());
        // Same host: select_host deliberately retains the old TerminalView.
        this.connect_core_using(cx, move |_| Ok(backend));
        this.terminal.link.inbox.clone()
    });
    // Force the race: publish the first connected core frame before any UI poll.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !inbox.lock().unwrap_or_else(|e| e.into_inner()).connected {
        assert!(Instant::now() < deadline, "core frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    wait_frame(&view, cx, |live| live.connected);
    view.update(cx, |this, _| {
        assert!(
            this.terminal.navigating.is_none(),
            "Herdr 1 navigation survived core open"
        );
        this.terminal
            .resize_if_ready(sheprd_client::ConnectOptions {
                surface_size: sheprd_client::protocol::ClientSurfaceSize {
                    cols: 100,
                    rows: 30,
                },
                ..Default::default()
            });
    });
    wait_frame(&view, cx, |live| {
        live.surface
            .as_ref()
            .is_some_and(|s| s.frame.width == 100 && s.frame.height == 30)
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "CORE", window, cx);
        })
    });
    assert_eq!(receive(&view, cx, &wire), (1, b"CORE".to_vec()));
    drop(release);
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
fn pane_close_preserves_final_frame_and_keeps_other_pane_live(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let wire = start(&view, cx);
    command(&view, cx, Command::Split);
    wait_frame(&view, cx, |live| {
        live.status.contains("2 pane(s)") && text(live).contains("pane 2")
    });
    command(&view, cx, Command::Focus(0));
    wait_frame(&view, cx, |live| live.status.contains("active 1"));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "exit\r", window, cx);
        })
    });
    assert_eq!(receive(&view, cx, &wire), (1, b"exit\r".to_vec()));
    wait_frame(&view, cx, |live| {
        live.status.contains("ended: 1") && text(live).contains("pane exited")
    });
    view.update(cx, |this, _| {
        assert!(
            this.terminal.live.connected,
            "live pane disconnected on sibling exit"
        );
        assert!(
            crate::terminal_input::destination(&this.terminal.live, this.terminal.link.generation)
                .is_none(),
            "ended pane accepts input"
        );
    });
    // Resize/focus must not call the ended terminal, but must still reach pane 2.
    view.update(cx, |this, _| {
        this.terminal
            .resize_if_ready(sheprd_client::ConnectOptions {
                surface_size: sheprd_client::protocol::ClientSurfaceSize {
                    cols: 100,
                    rows: 30,
                },
                ..Default::default()
            });
    });
    command(&view, cx, Command::Focus(1));
    wait_frame(&view, cx, |live| {
        live.status.contains("active 2")
            && live.surface.as_ref().is_some_and(|s| s.frame.width == 100)
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "SURVIVOR", window, cx);
        })
    });
    assert_eq!(receive(&view, cx, &wire), (2, b"SURVIVOR".to_vec()));
    wait_frame(&view, cx, |live| {
        live.connected && text(live).contains("SURVIVOR") && text(live).contains("pane exited")
    });
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
fn herdr1_mismatch_before_core_does_not_block_background_return(cx: &mut TestAppContext) {
    core_return_after_old_mismatch(cx, false);
}
#[gpui::test]
fn herdr1_mismatch_after_core_does_not_block_background_return(cx: &mut TestAppContext) {
    core_return_after_old_mismatch(cx, true);
}
fn core_return_after_old_mismatch(cx: &mut TestAppContext, late: bool) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let old_inbox = view.update(cx, |this, _| {
        this.select_host("private-core");
        this.terminal.link.inbox.clone()
    });
    let mismatch = || {
        old_inbox.lock().unwrap_or_else(|e| e.into_inner()).apply(
            sheprd_client::ClientEvent::VersionMismatch(
                sheprd_client::VersionMismatch::DaemonOutdated {
                    server_version: Some("old-herdr-1".into()),
                },
            ),
        );
    };
    if !late {
        mismatch();
    }
    let (backend, _) = crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let (release, wait) = mpsc::channel();
    view.update(cx, |this, cx| {
        this.connect_core_using(cx, move |_| {
            wait.recv().map_err(|e| crate::Error::Ssh(e.to_string()))?;
            Ok(backend)
        });
    });
    if late {
        mismatch();
    }
    release.send(()).unwrap_or_else(|e| panic!("release: {e}"));
    wait_frame(&view, cx, |live| live.connected && live.surface.is_some());
    // The retired reader really did retain its mismatch; only the new core
    // state must be isolated. No reader is allowed to clear another's state.
    assert!(
        old_inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .version_mismatch
            .get()
            .is_some()
    );
    let (backend, wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("return peer: {e}"));
    view.update(cx, |this, cx| {
        this.terminal.core_test_backend = Some(backend);
        this.lifecycle(gpui::AppLifecyclePhase::Background, cx);
        let paused = this.terminal.link.generation;
        this.lifecycle(gpui::AppLifecyclePhase::Active, cx);
        assert!(
            this.terminal.link.generation > paused,
            "old Herdr 1 mismatch blocked core lifecycle reconnect; late={late}"
        );
        assert!(
            this.terminal.core_test_backend.is_none(),
            "automatic core connector was not reached"
        );
        assert!(this.terminal.link.stop_reason.get().is_none());
    });
    wait_frame(&view, cx, |live| {
        live.connected && live.version_mismatch.get().is_none()
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, "RETURNED", window, cx);
        })
    });
    assert_eq!(receive(&view, cx, &wire), (1, b"RETURNED".to_vec()));
    wait_frame(&view, cx, |live| text(live).contains("RETURNED"));
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
fn sh1_155_inactive_keeps_core_identity_input_and_paint(cx: &mut TestAppContext) {
    for automatic in [true, false] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let wire = start(&view, cx);
        view.update(cx, |this, cx| {
            this.terminal.reconnect.automatic = automatic;
            let generation = this.terminal.link.generation;
            let inbox = this.terminal.link.inbox.clone();
            let finished = this
                .terminal
                .core
                .as_ref()
                .unwrap_or_else(|| panic!("core"))
                .finished
                .clone();
            for phase in [
                gpui::AppLifecyclePhase::Inactive,
                gpui::AppLifecyclePhase::Active,
            ] {
                this.lifecycle(phase, cx);
                assert!(!this.terminal.reconnect.suspended);
                let core = this
                    .terminal
                    .core
                    .as_ref()
                    .unwrap_or_else(|| panic!("core closed while inactive"));
                assert!(Arc::ptr_eq(&finished, &core.finished));
                assert!(Arc::ptr_eq(&inbox, &this.terminal.link.inbox));
                assert_eq!(generation, this.terminal.link.generation);
                assert!(this.terminal.live.connected);
            }
        });
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                this.refresh_terminal_input();
                this.terminal.input.fresh_gesture();
                this.replace_text_in_range(None, "INACTIVE", window, cx);
            })
        });
        assert_eq!(receive(&view, cx, &wire), (1, b"INACTIVE".to_vec()));
        wait_frame(&view, cx, |live| text(live).contains("INACTIVE"));
        view.update(cx, |this, cx| {
            this.lifecycle(gpui::AppLifecyclePhase::Background, cx);
            assert!(this.terminal.core.is_none());
            assert!(!this.terminal.live.connected);
            assert!(this.terminal.reconnect.suspended);
        });
    }
}
