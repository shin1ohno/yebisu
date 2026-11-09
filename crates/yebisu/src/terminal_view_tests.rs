use crate::{app::Smoke, mock_peer};
use gpui::{TestAppContext, canvas, div, point, prelude::*, px, size};
use sheprd_client::protocol::ClientMessage;
use std::time::{Duration, Instant};

#[gpui::test]
fn stale_paint_callback_cannot_resize_new_connection(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    let view = cx.new(Smoke::new);
    let stale = view.update(cx, |this, cx| {
        this.terminal.open = true;
        let a = this.terminal.link.reset();
        this.terminal.link.install(a, mock_peer::connect());
        this.terminal_paint_callback(cx)
    });
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("private peer: {e}"));
    view.update(cx, |this, _| {
        let b = this.terminal.link.reset();
        this.terminal.link.install(b, Ok(client));
        this.terminal.size = None;
        // Wait for the actual reader, without depending on the UI poll timer.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(update) = this.terminal.link.take_update()
                && update.surface.is_some()
            {
                this.terminal.live = update;
                break;
            }
            assert!(
                Instant::now() < deadline,
                "B reader did not deliver a surface"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    while receipts.try_recv().is_ok() {}
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        div()
            .size_full()
            .child(canvas(|_, _, _| (), stale).size_full())
    });
    view.update(cx, |this, _| {
        assert!(this.terminal.size.is_none(), "A's callback resized B");
    });
    assert!(
        !receipts
            .try_iter()
            .any(|message| matches!(message, ClientMessage::ClientShellResize { .. })),
        "stale callback sent resize to B"
    );
    // Positive control: the same adapter captured for B must send a real resize.
    let fresh = view.update(cx, |this, cx| this.terminal_paint_callback(cx));
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        canvas(|_, _, _| (), fresh).size_full()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let message = receipts
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e| panic!("B did not receive its resize: {e}"));
        if let ClientMessage::ClientShellResize { surface_size, .. } = message {
            view.update(cx, |this, _| {
                assert_eq!(
                    this.terminal.size.map(|s| s.surface_size),
                    Some(surface_size)
                )
            });
            break;
        }
    }
}
