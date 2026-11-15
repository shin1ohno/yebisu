use super::*;
use crate::core::Command;
use gpui::AppLifecyclePhase as Phase;

fn trigger(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext, mode: usize) {
    if mode == 0 {
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let g = this.terminal.geometry;
                this.selection_down(
                    &gpui::MouseDownEvent {
                        position: g.grid.origin
                            + gpui::point(
                                gpui::px(g.cell_width / 2.),
                                gpui::px(g.cell_height / 2.),
                            ),
                        button: gpui::MouseButton::Left,
                        click_count: 2,
                        modifiers: Default::default(),
                        first_mouse: false,
                    },
                    window,
                    cx,
                );
            })
        });
    } else if mode == 3 {
        for _ in 0..257 {
            key(view, cx, "overflow");
        }
    } else {
        view.update(cx, |this, _| match mode {
            1 => this.toggle_selection(),
            2 => {
                this.toggle_selection();
                this.toggle_selection();
            }
            4 => {
                this.actions.palette = true;
                this.sync_handoff();
            }
            5 => {
                this.terminal.navigating = Some(crate::board::Navigation {
                    boot: "c0ffee".into(),
                    revision: 0,
                    target: crate::board::Target::Command,
                    request: "test".into(),
                });
                this.sync_handoff();
            }
            6 => this.core_command(Command::Focus(0)),
            7 => this.core_command(Command::Split),
            8 => this.select_host("other-host"),
            9 => {
                this.select_host("other-host");
                this.select_host("cancel-host");
            }
            _ => panic!("mode"),
        });
    }
}
fn preparation_gate(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
) -> Arc<std::sync::Mutex<(bool, Option<std::task::Waker>)>> {
    let gate = Arc::new(std::sync::Mutex::new((false, None::<std::task::Waker>)));
    let waiting = gate.clone();
    let task = cx
        .background_executor
        .spawn(std::future::poll_fn(move |cx| {
            let mut g = waiting.lock().unwrap_or_else(|e| e.into_inner());
            if g.0 {
                std::task::Poll::Ready(())
            } else {
                g.1 = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        }));
    view.update(cx, |this, _| this.handoff.completion_gate = Some(task));
    gate
}

#[gpui::test]
fn cancellation_sources_fence_preparation_and_waiting_put(cx: &mut TestAppContext) {
    for preparing in [true, false] {
        for mode in 0..10 {
            let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
            let probe = Probe::default();
            probe.lock().replies.push_back(None);
            connect(&view, cx, probe.clone(), Control::default(), "cancel-host");
            let gate = preparing.then(|| preparation_gate(&view, cx));
            clipboard(cx, false);
            start(&view, cx, true);
            if preparing {
                cx.run_until_parked();
            } else {
                wait(&view, cx, |_| probe.lock().puts.len() == 1);
            }
            let serial = view.update(cx, |this, _| this.handoff.serial);
            trigger(&view, cx, mode);
            assert_ne!(
                view.update(cx, |this, _| this.handoff.serial),
                serial,
                "cancel source {mode}"
            );
            if let Some(gate) = gate {
                let mut g = gate.lock().unwrap_or_else(|e| e.into_inner());
                g.0 = true;
                if let Some(w) = g.1.take() {
                    w.wake();
                }
            } else {
                release(&probe, 0);
            }
            wait(&view, cx, |this| this.handoff.workers == 0);
            assert!(
                inputs(&probe).is_empty(),
                "misdelivery preparing={preparing} source={mode}"
            );
            assert_eq!(probe.lock().puts.len(), usize::from(!preparing));
        }
    }
}

#[gpui::test]
fn queued_cancel_preserves_already_released_keys_and_blocks_next_paste(cx: &mut TestAppContext) {
    for mode in [0, 1, 2, 4] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        hooks.before_input.arm();
        connect(&view, cx, probe.clone(), control.clone(), "cancel-host");
        view.update(cx, |this, _| this.handoff.core_test = Some(hooks.clone()));
        clipboard(cx, false);
        start(&view, cx, true);
        key(&view, cx, "one");
        key(&view, cx, "two");
        key(&view, cx, "three");
        wait(&view, cx, |_| {
            hooks.before_input.reached.load(Ordering::Acquire)
        });
        control.read.arm();
        wait(&view, cx, |_| control.read.reached.load(Ordering::Acquire));
        hooks.before_input.resume();
        wait(&view, cx, |this| this.handoff.workers == 0);
        start(&view, cx, true);
        assert_eq!(probe.lock().puts.len(), 1);
        trigger(&view, cx, mode);
        control.read.resume();
        wait(&view, cx, |_| probe.lock().inputs.len() == 3);
        assert_eq!(
            inputs(&probe),
            [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
    }
}

// Release the real old UI outcome only after the new connection is usable.
fn assert_reconnected_after_old_completion(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    old: &Probe,
    hooks: &TestHooks,
) {
    wait(view, cx, |_| {
        hooks.ui_completion.reached.load(Ordering::Acquire)
    });
    let new = Probe::default();
    connect(view, cx, new.clone(), Control::default(), "fresh-session");
    let (serial, generation, status) = view.update(cx, |this, _| {
        assert_eq!(this.handoff.workers, 1);
        assert!(this.handoff.held.is_empty());
        assert!(!this.handoff.preparing);
        assert!(this.terminal.live.connected);
        (
            this.handoff.serial,
            this.terminal.link.generation,
            this.terminal.live.status.clone(),
        )
    });
    assert!(inputs(old).is_empty());
    assert!(inputs(&new).is_empty());
    hooks.ui_completion.resume();
    wait(view, cx, |this| this.handoff.workers == 0);
    view.update(cx, |this, _| {
        assert_eq!(this.handoff.serial, serial, "old completion changed serial");
        assert_eq!(this.terminal.link.generation, generation);
        assert_eq!(
            this.terminal.live.status, status,
            "old completion changed new status"
        );
        assert!(this.handoff.held.is_empty());
        assert!(!this.handoff.preparing);
        assert!(this.terminal.live.connected);
    });
    assert!(inputs(old).is_empty());
    assert!(inputs(&new).is_empty());
    // Y29 cancelled via Select; leave that input-blocking mode explicitly.
    view.update(cx, |this, _| {
        if this.handoff.select_mode {
            this.toggle_selection();
        }
    });
    key(view, cx, "fresh");
    wait(view, cx, |_| new.lock().inputs.len() == 1);
    assert_eq!(inputs(&new), [b"fresh".to_vec()]);
    assert!(inputs(old).is_empty());
}

#[gpui::test]
fn partial_cancel_disconnects_and_finished_cancel_does_not_replay(cx: &mut TestAppContext) {
    for partial in [true, false] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        hooks.before_input.arm();
        if partial {
            hooks.ui_completion.arm();
        }
        connect(&view, cx, probe.clone(), control.clone(), "cancel-host");
        view.update(cx, |this, _| this.handoff.core_test = Some(hooks.clone()));
        clipboard(cx, false);
        start(&view, cx, true);
        if partial {
            key(&view, cx, "old-one");
            key(&view, cx, "old-two");
        }
        wait(&view, cx, |_| {
            hooks.before_input.reached.load(Ordering::Acquire)
        });
        if partial {
            control.writing.arm();
            control.write_mode.store(1, Ordering::Release);
        }
        hooks.before_input.resume();
        if partial {
            wait(&view, cx, |_| {
                control.writing.reached.load(Ordering::Acquire)
            });
        } else {
            wait(&view, cx, |this| {
                this.handoff.workers == 0 && probe.lock().inputs.len() == 1
            });
        }
        trigger(&view, cx, 1);
        if partial {
            control.writing.resume();
            wait(&view, cx, |this| {
                probe.eof.load(Ordering::Acquire) && !this.terminal.live.connected
            });
            assert_reconnected_after_old_completion(&view, cx, &probe, &hooks);
        } else {
            assert!(!probe.closed.load(Ordering::Acquire));
            assert_eq!(inputs(&probe).len(), 1);
        }
    }
}

#[gpui::test]
fn inactive_continues_but_background_closes_even_across_response_race(cx: &mut TestAppContext) {
    for mode in 0..3 {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        if mode != 0 {
            hooks.ui_completion.arm();
        }
        probe.lock().replies.push_back(None);
        connect(&view, cx, probe.clone(), control.clone(), "lifecycle");
        view.update(cx, |this, _| this.handoff.core_test = Some(hooks.clone()));
        clipboard(cx, false);
        start(&view, cx, true);
        if mode != 0 {
            key(&view, cx, "old-one");
            key(&view, cx, "old-two");
        }
        wait(&view, cx, |_| probe.lock().puts.len() == 1);
        if mode == 2 {
            control.after_read.arm();
            release(&probe, 0);
            wait(&view, cx, |_| {
                probe.replies_written.load(Ordering::Acquire) == 1
                    && control.after_read.reached.load(Ordering::Acquire)
            });
        }
        view.update(cx, |this, cx| {
            this.lifecycle(
                if mode == 0 {
                    Phase::Inactive
                } else {
                    Phase::Background
                },
                cx,
            )
        });
        if mode == 2 {
            control.after_read.resume();
        } else if mode == 0 {
            release(&probe, 0);
        }
        if mode == 0 {
            wait(&view, cx, |this| {
                this.handoff.workers == 0 && probe.lock().inputs.len() == 1
            });
        } else {
            // Y26 leaves put pending; Y31 has already written and read its reply.
            // Neither case may mistake a failed late response write for EOF.
            wait(&view, cx, |_| probe.eof.load(Ordering::Acquire));
            assert_eq!(
                probe.replies_written.load(Ordering::Acquire),
                usize::from(mode == 2)
            );
            assert!(inputs(&probe).is_empty());
            view.update(cx, |this, cx| {
                this.terminal.reconnect_pending = false;
                this.lifecycle(Phase::Active, cx);
            });
            assert_reconnected_after_old_completion(&view, cx, &probe, &hooks);
        }
    }
}

#[gpui::test]
fn old_completion_cannot_release_the_next_pastes_keys(cx: &mut TestAppContext) {
    for timeout in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        probe.lock().replies.extend([None, None]);
        connect(&view, cx, probe.clone(), Control::default(), "old");
        if timeout {
            // The timer starts at send and discards a put that arrives after it
            // expires, so leave room for a loaded host to deliver the put first.
            view.update(cx, |this, _| {
                this.handoff.core_timeout = Some(Duration::from_secs(6))
            });
        }
        clipboard(cx, false);
        start(&view, cx, true);
        wait(&view, cx, |_| probe.lock().puts.len() == 1);
        if !timeout {
            trigger(&view, cx, 1);
            start(&view, cx, true);
            assert_eq!(probe.lock().puts.len(), 1);
            release(&probe, 0);
        }
        wait(&view, cx, |this| this.handoff.workers == 0);
        view.update(cx, |this, _| {
            this.handoff.select_mode = false;
            this.terminal.input.fresh_gesture();
            this.handoff.core_timeout = None;
        });
        start(&view, cx, true);
        wait(&view, cx, |_| probe.lock().puts.len() == 2);
        key(&view, cx, "second-a");
        key(&view, cx, "second-b");
        if timeout {
            release(&probe, 0);
        }
        view.update(cx, |this, _| {
            assert!(this.handoff.preparing);
            assert_eq!(this.handoff.held.len(), 2);
        });
        release(&probe, 1);
        wait(&view, cx, |this| {
            this.handoff.workers == 0 && probe.lock().inputs.len() == 3
        });
        assert_eq!(
            inputs(&probe),
            [
                b"/released-1.png".to_vec(),
                b"second-a".to_vec(),
                b"second-b".to_vec()
            ]
        );
    }
}

#[gpui::test]
fn two_hosts_and_detach_do_not_deliver_old_paths(cx: &mut TestAppContext) {
    for detach in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let first = Probe::default();
        first.lock().replies.push_back(None);
        connect(&view, cx, first.clone(), Control::default(), "first");
        clipboard(cx, false);
        start(&view, cx, true);
        wait(&view, cx, |_| first.lock().puts.len() == 1);
        if detach {
            view.update(cx, |this, _| this.detach_core());
        }
        let second = Probe::default();
        connect(&view, cx, second.clone(), Control::default(), "second");
        release(&first, 0);
        wait(&view, cx, |this| this.handoff.workers == 0);
        assert!(inputs(&first).is_empty());
        assert!(inputs(&second).is_empty());
        view.update(cx, |this, _| this.select_host("first"));
        wait(&view, cx, |_| first.closed.load(Ordering::Acquire));
        let fresh = Probe::default();
        connect(&view, cx, fresh.clone(), Control::default(), "first");
        assert!(inputs(&fresh).is_empty());
        assert!(inputs(&first).is_empty());
    }
}

#[gpui::test]
fn queued_host_switch_closes_old_connection_without_stale_input_or_completion(
    cx: &mut TestAppContext,
) {
    for delayed_completion in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let first = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        hooks.before_input.arm();
        if delayed_completion {
            hooks.notification.arm();
        }
        connect(&view, cx, first.clone(), control.clone(), "first-queued");
        let finished = view.update(cx, |this, _| {
            this.handoff.core_test = Some(hooks.clone());
            this.terminal
                .core
                .as_ref()
                .unwrap_or_else(|| panic!("session"))
                .finished
                .clone()
        });
        clipboard(cx, false);
        start(&view, cx, true);
        key(&view, cx, "old-one");
        key(&view, cx, "old-two");
        key(&view, cx, "old-three");
        wait(&view, cx, |_| {
            hooks.before_input.reached.load(Ordering::Acquire)
        });
        control.read.arm();
        wait(&view, cx, |_| control.read.reached.load(Ordering::Acquire));
        hooks.before_input.resume();
        wait(&view, cx, |this| {
            this.handoff
                .core_ticket
                .as_ref()
                .and_then(|ticket| ticket.delivery())
                == Some(sheprd_core_client::Delivery::Queued)
        });
        if !delayed_completion {
            wait(&view, cx, |this| this.handoff.workers == 0);
        }
        let second = Probe::default();
        connect(
            &view,
            cx,
            second.clone(),
            Control::default(),
            "second-queued",
        );
        let (serial, generation) = view.update(cx, |this, _| {
            (this.handoff.serial, this.terminal.link.generation)
        });
        // Observe the old Session worker's exit, so disconnect must precede
        // the transport's resume instead of racing SDK queue drainage.
        wait(&view, cx, |_| finished.load(Ordering::Acquire));
        control.read.resume();
        if delayed_completion {
            hooks.notification.resume();
        }
        wait(&view, cx, |this| {
            first.eof.load(Ordering::Acquire) && this.handoff.workers == 0
        });
        assert!(inputs(&first).is_empty());
        assert!(inputs(&second).is_empty());
        view.update(cx, |this, _| {
            assert_eq!(this.handoff.serial, serial);
            assert_eq!(this.terminal.link.generation, generation);
            assert!(this.handoff.held.is_empty());
            assert!(this.terminal.live.connected);
            assert!(!this.handoff.preparing);
        });
        key(&view, cx, "fresh");
        wait(&view, cx, |_| second.lock().inputs.len() == 1);
        assert_eq!(inputs(&second), [b"fresh".to_vec()]);
    }
}
