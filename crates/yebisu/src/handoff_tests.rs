use super::*;
#[test]
fn rejected_image_is_not_published() {
    assert!(prepare(Payload::Image(b"not an image".to_vec())).is_err());
}
#[test]
fn large_image_is_bounded() -> crate::Result<()> {
    // JPEG COM segments are valid, ignored metadata, each with a 16-bit length.
    // They exceed the encoded-byte cap without exceeding the pixel/decode limits.
    let mut jpeg = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let jpeg = jpeg.into_inner();
    assert_eq!(&jpeg[..2], &[0xff, 0xd8]); // JPEG SOI
    let mut oversized = jpeg[..2].to_vec();
    while oversized.len() + jpeg.len() - 2 <= IMAGE_LIMIT {
        oversized.extend_from_slice(&[0xff, 0xfe, 0xff, 0xff]); // COM, length 65535
        oversized.resize(oversized.len() + usize::from(u16::MAX) - 2, 0);
    }
    oversized.extend_from_slice(&jpeg[2..]);
    assert!(oversized.len() > IMAGE_LIMIT);
    let decoded =
        image::load_from_memory(&oversized).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    assert_eq!((decoded.width(), decoded.height()), (1, 1));
    eprintln!(
        "valid oversized JPEG: bytes={} cap={IMAGE_LIMIT} decoded=1x1",
        oversized.len()
    );
    match prepare(Payload::Image(oversized)) {
        Err(crate::Error::Ssh(message)) => {
            assert_eq!(message, "Paste refused: image exceeds 128 MiB")
        }
        Ok(_) => panic!("valid over-cap image was accepted"),
        Err(error) => panic!("wrong image rejection: {error}"),
    }
    Ok(())
}
#[test]
fn host_switch_cancels_late_paste_and_following_input() {
    let d = Destination {
        generation: 1,
        epoch: 1,
        boot: "boot".into(),
        target: crate::terminal::InputTarget::Pane("pane".into()),
    };
    let mut h = Handoff::default();
    h.sync(Some(Stamp {
        selection: 1,
        destination: d.clone(),
    }));
    let serial = h.serial;
    h.preparing = true;
    assert!(h.hold(GeneratedInput {
        destination: d.clone(),
        event: ClientPaneInputEvent::Paste("after".into())
    }));
    h.sync(Some(Stamp {
        selection: 2,
        destination: d,
    }));
    assert_ne!(serial, h.serial);
    assert!(!h.preparing);
    assert!(h.held.is_empty());
}
#[test]
fn valid_image_round_trip_and_text_limit() -> crate::Result<()> {
    let source = image::DynamicImage::new_rgba8(32, 32);
    let mut out = std::io::Cursor::new(Vec::new());
    source
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let Prepared::Image(bytes) = prepare(Payload::Image(out.into_inner()))? else {
        panic!("expected image");
    };
    assert_eq!(
        image::load_from_memory(&bytes)
            .map_err(|e| crate::Error::Ssh(e.to_string()))?
            .width(),
        32
    );
    assert!(prepare(Payload::Text("a".repeat(TEXT_LIMIT + 1))).is_err());
    Ok(())
}

#[test]
fn large_supported_image_is_resized_under_wire_limit() -> crate::Result<()> {
    let mut state = 1u32;
    let image = image::RgbaImage::from_fn(2200, 2200, |_, _| {
        let mut c = [0; 4];
        for b in &mut c {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = state as u8;
        }
        image::Rgba(c)
    });
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    assert!(encoded.get_ref().len() > sheprd_client::protocol::MAX_CLIPBOARD_IMAGE_PAYLOAD);
    let Prepared::Image(bytes) = prepare(Payload::Image(encoded.into_inner()))? else {
        panic!("image expected");
    };
    assert!(bytes.len() <= sheprd_client::protocol::MAX_CLIPBOARD_IMAGE_PAYLOAD);
    let decoded = image::load_from_memory(&bytes).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    assert_eq!((decoded.width(), decoded.height()), (1100, 1100));
    Ok(())
}
#[test]
fn selection_skips_wide_continuation_and_normalizes_reverse() {
    let frame = Arc::new(crate::mock_peer::surface(7).frame);
    let forward = Selection {
        frame: frame.clone(),
        anchor: (1, 2),
        end: (6, 2),
        dragging: false,
    };
    let reverse = Selection {
        frame,
        anchor: (6, 2),
        end: (1, 2),
        dragging: false,
    };
    assert_eq!(forward.text().as_deref(), Some("日本語"));
    assert_eq!(forward.text(), reverse.text());
}
#[test]
fn cancelled_reservation_cannot_reach_another_peer() -> crate::Result<()> {
    use sheprd_client::protocol::{ClientClipboardImageTarget, ClientMessage};
    use std::time::{Duration, Instant};
    let (a, ra) = crate::mock_peer::connect_recorded(1)?;
    let (b, rb) = crate::mock_peer::connect_recorded(1)?;
    for client in [&a, &b] {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let event = client
                .events
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .map_err(|e| crate::Error::Ssh(e.to_string()))?;
            if matches!(event, sheprd_client::ClientEvent::Surface(_)) {
                break;
            }
        }
    }
    let upload = a
        .handle
        .reserve_clipboard_input(
            "boot-v1",
            ClientClipboardImageTarget::Pane("pane-v1".into()),
        )
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let mut h = Handoff {
        cancellation: Some(upload.cancellation_handle()),
        preparing: true,
        ..Default::default()
    };
    h.cancel();
    assert!(
        Prepared::Text("stale-paste".into())
            .publish(upload)
            .is_err()
    );
    // Barrier/positive control: B must receive its own paste, never A's.
    b.handle
        .send_input(
            "boot-v1",
            "pane-v1",
            [ClientPaneInputEvent::Paste("fresh-paste".into())],
        )
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let event = rb
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        if let ClientMessage::ClientShellPaneInput { events, .. } = event {
            assert_eq!(
                events,
                vec![ClientPaneInputEvent::Paste("fresh-paste".into())]
            );
            break;
        }
    }
    assert!(
        !ra.try_iter()
            .any(|m| matches!(m, ClientMessage::ClientShellPaneInput { .. }))
    );
    a.handle.disconnect();
    b.handle.disconnect();
    Ok(())
}

#[gpui::test]
fn busy_reader_is_not_a_target_change(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext;
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, _| {
        let mut live = crate::live::Live::default();
        live.apply(sheprd_client::ClientEvent::Snapshot(Arc::new(
            serde_json::from_str(crate::mock_peer::SNAPSHOT)
                .unwrap_or_else(|e| panic!("fixture: {e}")),
        )));
        live.apply(sheprd_client::ClientEvent::Surface(Arc::new(
            crate::mock_peer::surface(7),
        )));
        live.connected = true;
        this.terminal.open = true;
        this.terminal.link.inbox = Arc::new(std::sync::Mutex::new(live));
        this.sync_handoff();
        assert!(this.handoff.stamp.is_some());
        this.handoff.preparing = true;
        let serial = this.handoff.serial;
        let inbox = this.terminal.link.inbox.clone();
        let _lock = inbox.lock().unwrap_or_else(|e| panic!("lock: {e}"));
        this.sync_handoff();
        assert_eq!(this.handoff.serial, serial);
        assert!(this.handoff.preparing);
    });
}
#[test]
fn held_input_is_fifo_and_failure_discards_tail() {
    let d = Destination {
        generation: 1,
        epoch: 1,
        boot: "b".into(),
        target: crate::terminal::InputTarget::Pane("p".into()),
    };
    let mut h = Handoff::default();
    for text in ["one", "two"] {
        assert!(h.hold(GeneratedInput {
            destination: d.clone(),
            event: ClientPaneInputEvent::TextCommit(text.into())
        }));
    }
    assert_eq!(
        h.held.pop_front().map(|e| e.event),
        Some(ClientPaneInputEvent::TextCommit("one".into()))
    );
    h.cancel();
    assert!(h.held.is_empty());
}

#[test]
fn busy_paste_drops_only_the_tail_after_its_refusal() {
    let d = Destination {
        generation: 1,
        epoch: 1,
        boot: "b".into(),
        target: crate::terminal::InputTarget::Pane("p".into()),
    };
    let event = |text: &str| GeneratedInput {
        destination: d.clone(),
        event: ClientPaneInputEvent::TextCommit(text.into()),
    };
    let mut h = Handoff::default();
    assert!(h.hold(event("before-refusal")));
    h.refused_tail = true;
    assert!(!h.hold(event("after-refusal")));
    assert_eq!(h.held.len(), 1);
    assert_eq!(
        h.held.pop_front().map(|e| e.event),
        Some(event("before-refusal").event)
    );
}

#[test]
fn selecting_the_second_cell_of_a_wide_glyph_copies_the_whole_glyph() {
    let selection = Selection {
        frame: Arc::new(crate::mock_peer::surface(7).frame),
        anchor: (2, 2),
        end: (2, 2),
        dragging: false,
    };
    assert_eq!(selection.text().as_deref(), Some("日"));
    assert_eq!(selection.spans(), vec![(2, 1..3)]);
}
