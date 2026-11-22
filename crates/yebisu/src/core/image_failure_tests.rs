use super::*;
use crate::core::{Command, Session};
use gpui::AppContext;
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    mpsc,
};

#[gpui::test]
fn acquisition_failure_never_reserves_or_calls_put(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    connect(&view, cx, probe.clone(), Control::default(), "acquire");
    for image_only in [false, true] {
        crate::handoff_clipboard::INJECTED.with(|slot| {
            *slot.borrow_mut() = Some(
                crate::handoff_clipboard::admit(
                    1,
                    crate::handoff::IMAGE_LIMIT + 1,
                    crate::handoff::IMAGE_LIMIT,
                )
                .map(|_| Payload::Image(Vec::new())),
            )
        });
        admit(&view, cx, image_only);
        view.update(cx, |this, _| {
            assert_eq!(this.handoff.workers, 0);
            assert_eq!(this.terminal.live.status, "Paste exceeds acquisition limit");
        });
        assert!(probe.lock().puts.is_empty());
    }
}

#[gpui::test]
fn queue_full_and_dropped_reply_release_ui_slot(cx: &mut TestAppContext) {
    for full in [true, false] {
        // Only the upload belongs on the substitute queue.
        let view = cx.new(Smoke::new);
        let probe = Probe::default();
        connect(&view, cx, probe.clone(), Control::default(), "queue");
        let (queue, receive) = mpsc::sync_channel(1);
        if full {
            queue
                .send(crate::core::Queued {
                    command: Command::Orientation,
                    input_epoch: 0,
                })
                .unwrap_or_else(|e| panic!("queue:{e}"));
        }
        let old = view.update(cx, |this, _| {
            this.terminal.core.replace(Session {
                queue,
                input_epoch: Arc::new(AtomicU64::new(0)),
                stop: Arc::new(AtomicBool::new(false)),
                statuses: Default::default(),
                host: this.terminal.reconnect.target.clone(),
                name: "default".to_owned(),
                sessions: Default::default(),
                finished: Arc::new(AtomicBool::new(false)),
            })
        });
        clipboard(cx, false);
        admit(&view, cx, true);
        key(&view, cx, "discard");
        assert_eq!(view.update(cx, |this, _| this.handoff.held.len()), 1);
        if full {
            wait(&view, cx, |this| this.handoff.workers == 0);
            assert!(matches!(
                receive.try_recv(),
                Ok(crate::core::Queued {
                    command: Command::Orientation,
                    ..
                })
            ));
        } else {
            // Drop the reply for the put itself. A resize or focus command
            // taking this slot would exercise a different failure branch.
            wait(&view, cx, |_| match receive.try_recv() {
                Ok(crate::core::Queued {
                    command: Command::PutImage { .. },
                    ..
                }) => true,
                Err(mpsc::TryRecvError::Empty) => false,
                other => panic!("expected PutImage, got {:?}", other.map(|_| ())),
            });
            wait(&view, cx, |this| this.handoff.workers == 0);
            assert!(
                matches!(receive.try_recv(), Err(mpsc::TryRecvError::Empty)),
                "a dropped reply released input into the substitute queue"
            );
        }
        assert!(probe.lock().puts.is_empty());
        assert!(inputs(&probe).is_empty());
        view.update(cx, |this, _| {
            this.terminal.core = old;
            assert!(this.handoff.held.is_empty());
        });
        admit(&view, cx, true);
        wait(&view, cx, |this| {
            this.handoff.workers == 0 && probe.lock().inputs.len() == 1
        });
        assert_eq!(probe.lock().puts.len(), 1);
        assert_eq!(
            inputs(&probe),
            [br"/state\ with\ space/paste-0.png".to_vec()]
        );
    }
}

#[gpui::test]
fn timeout_branches_distinguish_unsent_written_and_partial_final_write(cx: &mut TestAppContext) {
    for mode in 0..5 {
        // The transport gates must see the image path write, with no paint
        // resize competing for the next write to the same socket.
        let view = cx.new(Smoke::new);
        let probe = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        if mode == 0 {
            probe.lock().replies.push_back(None);
        } else {
            hooks.before_input.arm();
            hooks.notification.arm();
        }
        connect(&view, cx, probe.clone(), control.clone(), "timeout");
        view.update(cx, |this, _| {
            this.handoff.core_test = Some(hooks.clone());
            this.handoff.core_timeout = Some(Duration::ZERO);
        });
        let expire = crate::core::paste_test_support::defer_outcome_timeout();
        clipboard(cx, false);
        admit(&view, cx, true);
        key(&view, cx, "held");
        assert_eq!(view.update(cx, |this, _| this.handoff.held.len()), 1);
        if mode == 0 {
            wait(&view, cx, |_| probe.lock().puts.len() == 1);
        } else {
            wait(&view, cx, |_| {
                hooks.before_input.reached.load(Ordering::Acquire)
            });
            if mode == 1 {
                control.read.arm();
                wait(&view, cx, |_| control.read.reached.load(Ordering::Acquire));
            }
            if mode >= 3 {
                control.writing.arm();
                control
                    .write_mode
                    .store(if mode == 3 { 1 } else { 2 }, Ordering::Release);
            }
            hooks.before_input.resume();
            let delivery = match mode {
                1 => sheprd_core_client::Delivery::Queued,
                2 => sheprd_core_client::Delivery::Written,
                _ => sheprd_core_client::Delivery::Writing,
            };
            wait(&view, cx, |this| {
                (mode < 3 || control.writing.reached.load(Ordering::Acquire))
                    && this
                        .handoff
                        .core_ticket
                        .as_ref()
                        .and_then(|ticket| ticket.delivery())
                        == Some(delivery)
                    && hooks.notification.reached.load(Ordering::Acquire)
            });
        }
        expire
            .send(())
            .unwrap_or_else(|e| panic!("start timeout: {e}"));
        wait(&view, cx, |_| hooks.timed_out.load(Ordering::Acquire));
        if mode >= 3 {
            control.writing.resume();
        }
        wait(&view, cx, |this| this.handoff.workers == 0);
        if mode == 1 {
            control.read.resume();
        }
        if mode == 0 {
            release(&probe, 0);
        } else {
            hooks.notification.resume();
        }
        let success = mode == 2 || mode == 4;
        if success {
            wait(&view, cx, |_| probe.lock().inputs.len() == 2);
            assert_eq!(
                inputs(&probe),
                [
                    br"/state\ with\ space/paste-0.png".to_vec(),
                    b"held".to_vec(),
                ],
                "mode {mode}"
            );
        } else {
            assert!(inputs(&probe).is_empty());
            assert!(view.update(cx, |this, _| this.handoff.held.is_empty()));
        }
        if mode == 3 {
            wait(&view, cx, |_| probe.eof.load(Ordering::Acquire));
        }
    }
}

#[gpui::test]
fn host_paths_and_bracketed_paste_reach_the_expected_terminal(cx: &mut TestAppContext) {
    for bracketed in [false, true] {
        for path in [
            "/日本語/paste-2.png",
            "/a b/'\"$",
            &format!("/{}", "a".repeat(3999)),
        ] {
            let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
            let probe = Probe {
                bracketed,
                ..Default::default()
            };
            probe
                .lock()
                .replies
                .push_back(Some(json!({"path":path,"bytes":png().len()})));
            connect(&view, cx, probe.clone(), Control::default(), "path");
            clipboard(cx, false);
            admit(&view, cx, true);
            wait(&view, cx, |this| {
                this.handoff.workers == 0 && probe.lock().inputs.len() == 1
            });
            let path = crate::core::paste_path::escape(path).unwrap_or_else(|e| panic!("path:{e}"));
            let expected = if bracketed {
                format!("\u{1b}[200~{path}\u{1b}[201~")
            } else {
                path
            };
            assert_eq!(inputs(&probe), [expected.into_bytes()]);
            let state = probe.lock();
            assert_eq!(
                state.puts[0]["request"]["params"]["terminal"],
                state.inputs[0].0
            );
        }
    }
}

#[gpui::test]
fn lost_transport_and_ended_terminal_never_paste(cx: &mut TestAppContext) {
    for ended in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        probe.lock().replies.push_back(None);
        connect(&view, cx, probe.clone(), Control::default(), "lost");
        clipboard(cx, false);
        admit(&view, cx, true);
        key(&view, cx, "discard");
        wait(&view, cx, |_| probe.lock().puts.len() == 1);
        if ended {
            probe.end.store(true, Ordering::Release);
            wait(&view, cx, |this| this.handoff.stamp.is_none());
            release(&probe, 0);
        } else {
            probe.close.store(true, Ordering::Release);
        }
        wait(&view, cx, |this| this.handoff.workers == 0);
        assert!(inputs(&probe).is_empty());
        if !ended {
            wait(&view, cx, |_| probe.closed.load(Ordering::Acquire));
        }
    }
}

#[gpui::test]
fn rpc_timeout_discards_late_success_and_allows_next_image(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    probe.lock().replies.push_back(None);
    connect(&view, cx, probe.clone(), Control::default(), "rpc-timeout");
    clipboard(cx, false);
    admit(&view, cx, true);
    key(&view, cx, "discard");
    wait(&view, cx, |_| probe.lock().puts.len() == 1);
    let deadline = Instant::now() + Duration::from_secs(16);
    wait(&view, cx, |this| {
        assert!(Instant::now() < deadline, "image RPC did not time out");
        this.handoff.workers == 0
    });
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.live.status.clone()),
        "Paste failed: host did not answer"
    );
    assert!(inputs(&probe).is_empty());
    release(&probe, 0);
    wait(&view, cx, |_| {
        probe.replies_written.load(Ordering::Acquire) == 1
    });
    admit(&view, cx, true);
    wait(&view, cx, |this| {
        this.handoff.workers == 0 && probe.lock().inputs.len() == 1
    });
    assert_eq!(
        inputs(&probe),
        [b"/state\\ with\\ space/paste-1.png".to_vec()]
    );
}

#[gpui::test]
fn real_socket_write_timeout_closes_connection_and_new_session_can_paste(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    let control = Control::default();
    control.write_chunk.store(4096, Ordering::Release);
    connect(&view, cx, probe.clone(), control, "write-timeout");
    probe.before_read.arm();
    wait(&view, cx, |_| {
        probe.before_read.reached.load(Ordering::Acquire)
    });
    let mut seed = 1u32;
    let pixels: Vec<u8> = (0..1024 * 512 * 3)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        })
        .collect();
    let source = image::RgbImage::from_raw(1024, 512, pixels).unwrap_or_else(|| panic!("pixels"));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(source)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("png:{e}"));
    assert!(bytes.get_ref().len() > 1024 * 1024);
    assert!(bytes.get_ref().len() < CORE_IMAGE_LIMIT);
    crate::handoff_clipboard::INJECTED.with(|slot| {
        *slot.borrow_mut() = Some(Ok(Payload::Image(bytes.into_inner())));
    });
    admit(&view, cx, true);
    key(&view, cx, "discard");
    cx.run_until_parked();
    // Keep the peer stopped until the socket timeout closes the connection.
    let deadline = Instant::now() + Duration::from_secs(12);
    wait(&view, cx, |this| {
        assert!(Instant::now() < deadline, "socket write did not time out");
        this.handoff.workers == 0 && !this.terminal.live.connected
    });
    assert!(inputs(&probe).is_empty());
    assert!(probe.lock().puts.is_empty());
    view.update(cx, |this, cx| {
        assert!(this.handoff.held.is_empty());
        assert!(!this.terminal.live.connected);
        let reads = this.handoff.clipboard_reads;
        this.start_paste(true, cx);
        assert_eq!(reads, this.handoff.clipboard_reads);
    });
    probe.before_read.resume();
    wait(&view, cx, |_| probe.closed.load(Ordering::Acquire));
    let fresh = Probe::default();
    connect(
        &view,
        cx,
        fresh.clone(),
        Control::default(),
        "fresh-after-timeout",
    );
    assert!(inputs(&fresh).is_empty());
    clipboard(cx, false);
    admit(&view, cx, true);
    wait(&view, cx, |this| {
        this.handoff.workers == 0 && fresh.lock().inputs.len() == 1
    });
}
