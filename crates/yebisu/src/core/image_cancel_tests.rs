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
    for mode in [0, 1, 4, 8] {
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

#[gpui::test]
fn partial_cancel_disconnects_and_finished_cancel_does_not_replay(cx: &mut TestAppContext) {
    for partial in [true, false] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        let control = Control::default();
        let hooks = Arc::new(TestHooks::default());
        hooks.before_input.arm();
        connect(&view, cx, probe.clone(), control.clone(), "cancel-host");
        view.update(cx, |this, _| this.handoff.core_test = Some(hooks.clone()));
        clipboard(cx, false);
        start(&view, cx, true);
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
                probe.closed.load(Ordering::Acquire) && this.handoff.workers == 0
            });
            assert!(inputs(&probe).is_empty());
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
        probe.lock().replies.push_back(None);
        connect(&view, cx, probe.clone(), control.clone(), "lifecycle");
        clipboard(cx, false);
        start(&view, cx, true);
        wait(&view, cx, |_| probe.lock().puts.len() == 1);
        if mode == 2 {
            control.after_read.arm();
            release(&probe, 0);
            wait(&view, cx, |_| {
                control.after_read.reached.load(Ordering::Acquire)
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
        } else {
            release(&probe, 0);
        }
        wait(&view, cx, |this| this.handoff.workers == 0);
        if mode == 0 {
            wait(&view, cx, |_| probe.lock().inputs.len() == 1);
        } else {
            wait(&view, cx, |_| probe.closed.load(Ordering::Acquire));
            assert!(inputs(&probe).is_empty());
            view.update(cx, |this, cx| {
                this.terminal.reconnect_pending = false;
                this.lifecycle(Phase::Active, cx);
            });
            let new = Probe::default();
            connect(&view, cx, new.clone(), Control::default(), "fresh-session");
            assert!(inputs(&new).is_empty());
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
            view.update(cx, |this, _| {
                this.handoff.core_timeout = Some(Duration::from_millis(200))
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
