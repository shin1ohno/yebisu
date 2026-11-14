use super::*;

#[test]
fn saved_workspace_rejects_another_host() -> crate::Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/yebisu-core-store-{}",
        std::process::id()
    ));
    let saved = store::Saved {
        ids: vec![sheprd_backend::TerminalId::new("t1")?],
        active: 0,
        vertical: true,
    };
    store::save(Some(&root), "alpha", &saved)?;
    assert_eq!(store::load(Some(&root), "alpha")?.ids, saved.ids);
    assert!(store::load(Some(&root), "beta")?.ids.is_empty());
    std::fs::remove_dir_all(root)?;
    Ok(())
}

use crate::app::Smoke;
use gpui::{EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, TestAppContext};
use std::time::{Duration, Instant};

#[gpui::test]
fn old_tunnel_completion_cannot_replace_new_host_view(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (release, wait) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    let old = view.update(cx, |this, _| {
        this.select_host("old-host");
        let generation = this.terminal.link.reset();
        let inbox = this.terminal.link.inbox.clone();
        this.terminal.core = Some(
            Session::start(
                "old-host".into(),
                generation,
                inbox.clone(),
                move || {
                    let _ = wait.recv();
                    let _ = finished.send(());
                    Err(crate::Error::Ssh("late old tunnel failure".into()))
                },
                None,
            )
            .unwrap_or_else(|e| panic!("start: {e}")),
        );
        inbox
    });
    view.update(cx, |this, _| {
        this.select_host("new-host");
        let mut live = this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        live.status = "new host view".into();
        live.dirty = true;
    });
    release.send(()).unwrap_or_else(|e| panic!("release: {e}"));
    done.recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("completion: {e}"));
    let deadline = Instant::now() + Duration::from_secs(30);
    while !old
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .status
        .contains("late old tunnel")
    {
        assert!(Instant::now() < deadline, "old completion not delivered");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        assert_eq!(
            this.terminal
                .link
                .inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .status,
            "new host view"
        );
        this.poll_terminal(cx);
        assert_eq!(this.terminal.reconnect.target, "new-host");
        assert_eq!(this.terminal.live.status, "new host view");
    });
}

#[gpui::test]
fn core_wire_receives_uikit_callbacks_for_all_input_options(cx: &mut TestAppContext) {
    // 4 runtime switches x both host keyboard-reporting modes, through Smoke's
    // actual key and EntityInputHandler callbacks, then the core wire encoder.
    for bits in 0..32 {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let report = bits & 16 != 0;
        let (backend, receipts) =
            crate::core_mock::recorded(report).unwrap_or_else(|e| panic!("mock: {e}"));
        view.update(cx, |this, _| {
            this.select_host("private-core");
            this.terminal.core_mode = true;
            let generation = this.terminal.link.reset();
            this.terminal.core = Some(
                Session::start(
                    "private-core".into(),
                    generation,
                    this.terminal.link.inbox.clone(),
                    move || Ok(backend),
                    None,
                )
                .unwrap_or_else(|e| panic!("start: {e}")),
            );
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.connected && this.terminal.live.keyboard_report_all == report
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "no frame bits={bits}");
            std::thread::sleep(Duration::from_millis(5));
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("core-clipboard".into()));
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                this.terminal.input.options.option_alt = bits & 1 != 0;
                this.terminal.input.options.command_control = bits & 2 != 0;
                this.terminal.input.options.caret_anchor = bits & 4 != 0;
                this.actions.terminal_priority = bits & 8 != 0;
                this.terminal.size = None;
                this.refresh_terminal_input();
                this.terminal.input.fresh_gesture();
                this.replace_and_mark_text_in_range(None, "日本語", Some(1..2), window, cx);
                assert_eq!(this.terminal.input.composition.selected, 1..2);
                this.terminal.geometry = crate::input_geometry::InputGeometry {
                    grid: gpui::Bounds::new(
                        gpui::point(gpui::px(0.), gpui::px(0.)),
                        gpui::size(gpui::px(640.), gpui::px(480.)),
                    ),
                    cell_width: 8.,
                    cell_height: 20.,
                };
                let bounds = this
                    .bounds_for_range(1..3, this.terminal.geometry.grid, window, cx)
                    .unwrap_or_else(|| panic!("candidate bounds"));
                assert_eq!(bounds.size.height, gpui::px(20.));
                if bits & 4 != 0 {
                    assert_eq!(bounds.size.width, gpui::px(8.));
                } else {
                    assert!(
                        bounds.size.width > gpui::px(8.),
                        "clause must cover both characters"
                    );
                }
                assert!(bounds.left() >= gpui::px(0.) && bounds.right() <= gpui::px(640.));
                assert!(receipts.try_recv().is_err(), "uncommitted IME escaped");
                this.replace_text_in_range(None, "日本語", window, cx);
                for (key, modifiers) in [
                    (
                        "x",
                        Modifiers {
                            alt: true,
                            ..Modifiers::default()
                        },
                    ),
                    (
                        "g",
                        Modifiers {
                            platform: true,
                            ..Modifiers::default()
                        },
                    ),
                ] {
                    let keystroke = Keystroke {
                        key: key.into(),
                        key_char: Some(key.into()),
                        modifiers,
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
                }
                let paste = Keystroke {
                    key: "v".into(),
                    key_char: Some("v".into()),
                    modifiers: Modifiers {
                        platform: true,
                        ..Modifiers::default()
                    },
                };
                this.terminal_key_down(
                    &KeyDownEvent {
                        keystroke: paste.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
                this.terminal_key_up(&KeyUpEvent { keystroke: paste }, window, cx);
                // The UIKit native Paste selector is routed to this same entry,
                // independently of Command-as-Control and shortcut priority.
                this.start_paste(false, cx);
                assert_eq!(
                    this.handoff.clipboard_reads,
                    if bits & 10 == 10 { 1 } else { 2 }
                );
                let reads = this.handoff.clipboard_reads;
                this.start_paste(true, cx);
                assert_eq!(
                    this.handoff.clipboard_reads,
                    reads + 1,
                    "image-only acquisition checks for an image even when only text is present"
                );
                // Fence all prior bytes with a final UIKit text commit.
                this.replace_text_in_range(None, "END", window, cx);
            })
        });
        let mut bytes = Vec::new();
        loop {
            let (channel, chunk) = receipts
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|e| panic!("wire bits={bits}: {e}, {bytes:?}"));
            assert_eq!(channel, 1);
            bytes.extend(chunk);
            if bytes.ends_with(b"END") {
                break;
            }
        }
        // Expectations: sheprd-terminal input.rs:54 (UTF-8 commit) and
        // input/tests/kitty.rs:24,50-73 (Alt, Ctrl, release encoding), at pinned rev.
        let mut expected = "日本語".as_bytes().to_vec();
        if bits & 1 != 0 {
            expected.extend_from_slice(if report {
                b"\x1b[120;3u\x1b[120;3:3u"
            } else {
                b"\x1bx"
            });
        }
        if bits & 2 != 0 {
            expected.extend_from_slice(if report {
                b"\x1b[103;5u\x1b[103;5:3u"
            } else {
                b"\x07"
            });
        }
        if bits & 10 == 10 {
            expected.extend_from_slice(if report {
                b"\x1b[118;5u\x1b[118;5:3u"
            } else {
                b"\x16"
            });
        } else {
            expected.extend_from_slice(b"core-clipboard");
        }
        expected.extend_from_slice(b"core-clipboardEND");
        assert_eq!(bytes, expected, "option bits={bits}");
        view.update(cx, |this, _| {
            this.terminal.core.take();
        });
    }
}

#[gpui::test]
fn split_focus_and_reattach_preserve_ids_and_reject_old_input(cx: &mut TestAppContext) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/core-reattach-{}", std::process::id()));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let host = crate::core_mock::Host::default();
    // Begin with a previously saved terminal, not a newly spawned workspace.
    store::save(
        Some(&root),
        "core-split",
        &store::Saved {
            ids: vec![sheprd_backend::TerminalId::new("t1").unwrap_or_else(|e| panic!("id: {e}"))],
            active: 0,
            vertical: false,
        },
    )
    .unwrap_or_else(|e| panic!("save: {e}"));
    for reattach in [false, true] {
        let (backend, receipts) = host.recorded(false).unwrap_or_else(|e| panic!("mock: {e}"));
        view.update(cx, |this, _| {
            this.select_host("core-split");
            this.terminal.core_mode = true;
            let generation = this.terminal.link.reset();
            this.terminal.core = Some(
                Session::start(
                    "core-split".into(),
                    generation,
                    this.terminal.link.inbox.clone(),
                    move || Ok(backend),
                    Some(root.clone()),
                )
                .unwrap_or_else(|e| panic!("start: {e}")),
            );
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.connected
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "no initial frame");
            std::thread::sleep(Duration::from_millis(5));
        }
        if !reattach {
            view.update(cx, |this, _| {
                let old = crate::terminal_input::destination(
                    &this.terminal.live,
                    this.terminal.link.generation,
                )
                .unwrap_or_else(|| panic!("destination"));
                let core = this
                    .terminal
                    .core
                    .as_ref()
                    .unwrap_or_else(|| panic!("core"));
                core.send(Command::Split)
                    .unwrap_or_else(|e| panic!("split {e}"));
                core.send(Command::Orientation)
                    .unwrap_or_else(|e| panic!("layout {e}"));
                core.send(Command::Input(
                    old,
                    ClientPaneInputEvent::TextCommit("STALE".into()),
                ))
                .unwrap_or_else(|e| panic!("input {e}"));
            });
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if view.update(cx, |this, cx| {
                this.poll_terminal(cx);
                this.terminal.live.status.contains("2 pane(s)")
                    && this.terminal.live.status.contains("Stacked")
                    && this.terminal.live.connected
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "split or restore absent");
            std::thread::sleep(Duration::from_millis(5));
        }
        let saved = store::load(Some(&root), "core-split").unwrap_or_else(|e| panic!("saved: {e}"));
        assert_eq!(saved.ids.len(), 2);
        assert_ne!(
            saved.ids[0], saved.ids[1],
            "split must create a distinct terminal"
        );
        assert_eq!(saved.ids[0].as_str(), "t1");
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.terminal_focus.focus(window, cx);
                this.terminal.size = None;
                this.refresh_terminal_input();
                this.terminal.input.fresh_gesture();
                this.replace_text_in_range(None, "PANE2", window, cx);
            })
        });
        assert_eq!(
            receipts
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|e| panic!("wire {e}")),
            (2, b"PANE2".to_vec())
        );
        let finished = view.update(cx, |this, _| {
            let core = this
                .terminal
                .core
                .take()
                .unwrap_or_else(|| panic!("session"));
            core.finished.clone()
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        while !finished.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline, "worker did not stop");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    std::fs::remove_dir_all(root).unwrap_or_else(|e| panic!("cleanup {e}"));
}

#[test]
fn run_after_saved_attach_never_reuses_existing_terminal_id() -> crate::Result<()> {
    use sheprd_backend::{Backend as _, Role, Size, Spawn};
    let (backend, _) = crate::core_mock::recorded(false)?;
    let existing = backend.terminals()?;
    let saved = existing.first().ok_or(crate::Error::CoreState)?.id.clone();
    let _attachment = backend.attach(&saved, Size::new(80, 24)?, Role::Active)?;
    let created = backend.spawn(&Spawn::default())?;
    assert!(
        existing.iter().all(|terminal| terminal.id != created.id),
        "new split terminal {} duplicates a saved/existing terminal",
        created.id
    );
    assert!(
        backend
            .terminals()?
            .iter()
            .any(|terminal| terminal.id == created.id)
    );
    Ok(())
}
