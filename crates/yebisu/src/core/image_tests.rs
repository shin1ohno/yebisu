use crate::{
    app::Smoke,
    core_mock::{self, paste::Probe, transport::Control},
    handoff::{CORE_IMAGE_LIMIT, Payload, Prepared},
    handoff_ticket::TestHooks,
    terminal_input::GeneratedInput,
};
use gpui::{ClipboardEntry, ClipboardItem, Entity, TestAppContext};
use serde_json::json;
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(24, 32)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("png: {e}"));
    bytes.into_inner()
}
fn clipboard(cx: &mut TestAppContext, both: bool) {
    let mut entries = vec![ClipboardEntry::Image(gpui::Image::from_bytes(
        gpui::ImageFormat::Png,
        png(),
    ))];
    if both {
        entries.insert(0, ClipboardEntry::String("shadow".to_owned().into()));
    }
    cx.write_to_clipboard(ClipboardItem { entries });
}
#[track_caller]
fn wait(view: &Entity<Smoke>, cx: &mut TestAppContext, mut ready: impl FnMut(&Smoke) -> bool) {
    wait_for(view, cx, |this, _| ready(this));
}

#[track_caller]
fn wait_for(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    mut ready: impl FnMut(&mut Smoke, &mut gpui::Context<Smoke>) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        super::paste_test_support::pump();
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            ready(this, cx)
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition failed, status={}",
            view.update(cx, |this, _| this.terminal.live.status.clone())
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn connect(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    probe: Probe,
    control: Control,
    host: &str,
) {
    let (backend, _) =
        core_mock::controlled(probe, control).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, cx| {
        this.setup_input.text = host.into();
        this.terminal.open = false;
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait_for(view, cx, |this, _| {
        this.terminal.live.connected && input_frame_ready(this)
    });
}
fn start(view: &Entity<Smoke>, cx: &mut TestAppContext, image_only: bool) {
    view.update(cx, |this, cx| this.start_paste(image_only, cx));
}
// Observe the mailbox and the latest resize in the same UI update as the
// gesture. A connected frame can belong to the previous viewport.
fn input_frame_ready(this: &mut Smoke) -> bool {
    let terminal = &mut this.terminal;
    let Ok(live) = terminal.link.inbox.try_lock() else {
        return false;
    };
    terminal
        .input
        .sync(&live, terminal.link.generation, terminal.size);
    terminal.input.ready
}

#[track_caller]
fn admit(view: &Entity<Smoke>, cx: &mut TestAppContext, image_only: bool) {
    wait_for(view, cx, |this, cx| {
        if !input_frame_ready(this)
            || this.handoff.workers != 0
            || this
                .handoff
                .core_ticket
                .as_ref()
                .is_some_and(|ticket| !ticket.is_finished())
        {
            return false;
        }
        let before = this.handoff.clipboard_reads;
        this.start_paste(image_only, cx);
        assert_eq!(
            this.handoff.clipboard_reads - before,
            1,
            "paste not admitted, status={}",
            this.terminal.live.status
        );
        true
    });
}

fn key(view: &Entity<Smoke>, cx: &mut TestAppContext, text: &str) {
    view.update(cx, |this, _| {
        let destination = this
            .handoff
            .stamp
            .as_ref()
            .unwrap_or_else(|| panic!("stamp"))
            .destination
            .clone();
        this.send_terminal(GeneratedInput {
            destination,
            event: sheprd_backend::ClientPaneInputEvent::Paste(text.into()),
        });
    });
}
fn inputs(probe: &Probe) -> Vec<Vec<u8>> {
    probe
        .lock()
        .inputs
        .iter()
        .map(|(_, bytes)| bytes.clone())
        .collect()
}
fn release(probe: &Probe, index: usize) {
    let mut state = probe.lock();
    let bytes = state.puts[index]["bytes"]
        .as_array()
        .unwrap_or_else(|| panic!("bytes"))
        .len();
    state.releases.push((
        index,
        json!({"path":format!("/released-{index}.png"),"bytes":bytes}),
    ));
}

#[gpui::test]
fn image_clipboard_uses_put_then_one_path_and_text_has_priority(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    connect(&view, cx, probe.clone(), Control::default(), "image-host");
    // Cumulative puts and peer-recorded inputs after each paste. The peer thread
    // records an input after the paste completes, so wait for the exact count.
    for (both, image_only, puts, inputs) in [
        (false, true, 1, 1),
        (false, false, 2, 2),
        (true, false, 2, 3),
        (true, true, 3, 4),
    ] {
        clipboard(cx, both);
        admit(&view, cx, image_only);
        wait(&view, cx, |this| {
            this.handoff.workers == 0 && probe.lock().inputs.len() >= inputs
        });
        assert_eq!(probe.lock().puts.len(), puts);
    }
    let state = probe.lock();
    assert_eq!(state.inputs.len(), 4);
    assert_eq!(state.inputs[2].1, b"shadow");
    for (index, put) in state.puts.iter().enumerate() {
        assert_eq!(put["request"]["params"]["name"], "paste.png");
        let bytes: Vec<u8> =
            serde_json::from_value(put["bytes"].clone()).unwrap_or_else(|e| panic!("bytes:{e}"));
        let image = image::load_from_memory(&bytes).unwrap_or_else(|e| panic!("image:{e}"));
        assert_eq!((image.width(), image.height()), (24, 32));
        assert!(bytes.len() <= CORE_IMAGE_LIMIT);
        let path = format!("/state\\ with\\ space/paste-{index}.png").into_bytes();
        assert_eq!(state.inputs.iter().filter(|(_, b)| *b == path).count(), 1);
    }
}

#[gpui::test]
fn pending_put_holds_keys_fifo_and_refuses_double_tap(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    probe.lock().replies.push_back(None);
    connect(&view, cx, probe.clone(), Control::default(), "fifo");
    clipboard(cx, false);
    admit(&view, cx, true);
    wait(&view, cx, |_| probe.lock().puts.len() == 1);
    for text in ["a", "b", "c"] {
        key(&view, cx, text);
    }
    start(&view, cx, true);
    assert_eq!(probe.lock().puts.len(), 1);
    assert!(inputs(&probe).is_empty());
    release(&probe, 0);
    wait(&view, cx, |this| {
        this.handoff.workers == 0 && probe.lock().inputs.len() == 4
    });
    assert_eq!(
        inputs(&probe),
        [
            b"/released-0.png".to_vec(),
            b"a".to_vec(),
            b"b".to_vec(),
            b"c".to_vec()
        ]
    );
}

#[gpui::test]
fn refused_and_invalid_puts_release_slot_discard_keys_and_allow_new_paste(cx: &mut TestAppContext) {
    let replies = vec![
        json!({"path":"/p"}),
        json!({"path":"/p","bytes":"bad"}),
        json!({"path":"/p","bytes":999}),
        json!({"path":"relative","bytes":png().len()}),
        json!({"path":"/bad\npath","bytes":png().len()}),
        json!({"path":"/bad\u{1b}path","bytes":png().len()}),
        json!({"path":format!("/{}", "x".repeat(4096)),"bytes":png().len()}),
    ];
    let errors = [
        "finished",
        "not_found",
        "machine_unknown",
        "invalid_params",
        "internal",
    ]
    .into_iter()
    .map(|kind| json!({"error":{"code":-32000,"message":"injected","data":{"kind":kind}}}));
    for reply in replies.into_iter().chain(errors) {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe::default();
        probe.lock().replies.push_back(None);
        connect(&view, cx, probe.clone(), Control::default(), "failure");
        clipboard(cx, false);
        admit(&view, cx, true);
        key(&view, cx, "discard");
        assert_eq!(view.update(cx, |this, _| this.handoff.held.len()), 1);
        wait(&view, cx, |_| probe.lock().puts.len() == 1);
        probe.lock().releases.push((0, reply));
        wait(&view, cx, |this| this.handoff.workers == 0);
        assert!(inputs(&probe).is_empty());
        assert!(view.update(cx, |this, _| this.handoff.held.is_empty()));

        admit(&view, cx, true);
        wait(&view, cx, |this| {
            this.handoff.workers == 0 && !probe.lock().inputs.is_empty()
        });
        assert_eq!(probe.lock().puts.len(), 2);
        assert_eq!(
            inputs(&probe),
            [br"/state\ with\ space/paste-1.png".to_vec()]
        );
    }
}

#[gpui::test]
fn optional_method_and_injected_busy_fail_without_retry(cx: &mut TestAppContext) {
    for mode in 0..3 {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        let probe = Probe {
            advertise: mode != 0,
            ..Default::default()
        };
        connect(&view, cx, probe.clone(), Control::default(), "busy");
        let hooks = Arc::new(TestHooks::default());
        hooks.put_busy.store(mode == 1, Ordering::Release);
        hooks.input_busy.store(mode == 2, Ordering::Release);
        view.update(cx, |this, _| this.handoff.core_test = Some(hooks));
        clipboard(cx, false);
        admit(&view, cx, true);
        key(&view, cx, "discard");
        wait(&view, cx, |this| this.handoff.workers == 0);
        assert!(inputs(&probe).is_empty());
        assert_eq!(probe.lock().puts.len(), usize::from(mode == 2));
        if mode != 0 {
            admit(&view, cx, true);
            wait(&view, cx, |this| {
                this.handoff.workers == 0 && probe.lock().inputs.len() == 1
            });
        }
    }
}

#[test]
fn image_limit_boundary_and_acquisition_cap_are_exact() -> crate::Result<()> {
    let source = png();
    let n = source.len();
    let Prepared::Image(full) = crate::handoff::prepare(Payload::Image(source.clone()), n)? else {
        panic!("image")
    };
    assert_eq!(
        image::load_from_memory(&full)
            .map_err(|e| crate::Error::Ssh(e.to_string()))?
            .width(),
        24
    );
    let Prepared::Image(small) = crate::handoff::prepare(Payload::Image(source), n - 1)? else {
        panic!("image")
    };
    assert!(
        image::load_from_memory(&small)
            .map_err(|e| crate::Error::Ssh(e.to_string()))?
            .width()
            < 24
    );
    assert!(
        crate::handoff_clipboard::admit(
            1,
            crate::handoff::IMAGE_LIMIT,
            crate::handoff::IMAGE_LIMIT
        )
        .is_ok()
    );
    assert!(
        crate::handoff_clipboard::admit(
            1,
            crate::handoff::IMAGE_LIMIT + 1,
            crate::handoff::IMAGE_LIMIT
        )
        .is_err()
    );
    assert!(crate::handoff::prepare(Payload::Image(png()), 0).is_err());
    Ok(())
}

#[test]
fn returned_paths_reject_controls_and_escape_shell_ascii() {
    for path in [
        "",
        "relative",
        "/line\nbreak",
        "/esc\u{1b}",
        "/del\u{7f}",
        "/c1\u{80}",
    ] {
        assert!(super::paste_path::escape(path).is_err());
    }
    assert!(super::paste_path::escape(&format!("/{}", "a".repeat(4096))).is_err());
    for path in ["/日本語/paste-2.png", "/a_b-+,.:@%"] {
        assert_eq!(super::paste_path::escape(path), Ok(path.into()));
    }
    assert_eq!(
        super::paste_path::escape("/a b/'\"$"),
        Ok("/a\\ b/\\'\\\"\\$".into())
    );
    assert_eq!(
        super::paste_path::escape(&format!("/{}", "a".repeat(3999))).map(|s| s.len()),
        Ok(4000)
    );
}

#[path = "image_cancel_tests.rs"]
mod cancellations;
#[path = "image_failure_tests.rs"]
mod failures;

#[test]
fn empty_corrupt_unsupported_and_excess_pixel_images_are_refused() {
    for bytes in [Vec::new(), b"garbage".to_vec(), b"<svg/>".to_vec()] {
        assert!(crate::handoff::prepare(Payload::Image(bytes), CORE_IMAGE_LIMIT).is_err());
    }
    let mut bytes = png();
    bytes[16..20].copy_from_slice(&8193u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&8193u32.to_be_bytes());
    bytes[25] = 0; // grayscale bounds the decoder's header allocation below 256 MiB.
    let mut crc = !0u32;
    for byte in &bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    match crate::handoff::prepare(Payload::Image(bytes), CORE_IMAGE_LIMIT) {
        Err(crate::Error::Ssh(message)) => {
            assert_eq!(message, "Paste refused: image exceeds 64 Mi pixels")
        }
        _ => panic!("pixel cap must reject before decoding image data"),
    }
}

#[gpui::test]
fn command_v_with_only_image_uses_the_full_machine_address(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let probe = Probe::default();
    let (backend, _) =
        core_mock::named(probe.clone(), "machine/t7").unwrap_or_else(|e| panic!("peer:{e}"));
    view.update(cx, |this, cx| {
        this.setup_input.text = "remote".into();
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(&view, cx, |this| this.terminal.live.connected);
    clipboard(cx, false);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.terminal.size = None;
            let key = gpui::Keystroke {
                key: "v".into(),
                key_char: Some("v".into()),
                modifiers: gpui::Modifiers {
                    platform: true,
                    ..Default::default()
                },
            };
            this.terminal_key_down(
                &gpui::KeyDownEvent {
                    keystroke: key.clone(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            this.terminal_key_up(&gpui::KeyUpEvent { keystroke: key }, window, cx);
        })
    });
    wait(&view, cx, |this| {
        this.handoff.workers == 0 && probe.lock().inputs.len() == 1
    });
    let state = probe.lock();
    assert_eq!(state.puts.len(), 1);
    assert_eq!(state.puts[0]["request"]["params"]["terminal"], "machine/t7");
    assert_eq!(state.inputs[0].0, "machine/t7");
}
