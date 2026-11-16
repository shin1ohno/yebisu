//! A retired search field stays GPUI's platform input handler until the first
//! post-close draw. Its callbacks reach the terminal input path only while
//! search is closed, the retirement fence is open and no draw has happened;
//! otherwise it keeps its inert retired reply and sends nothing.
#![allow(clippy::unwrap_used)]
use super::available;
use crate::{app::Smoke, find::field::Field};
use gpui::{
    Bounds, ElementInputHandler, Entity, InputHandler, TestAppContext, VisualTestContext, Window,
};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

fn install(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Receiver<ClientMessage> {
    let (client, receipts) = crate::find::peer::connect().unwrap();
    view.update(cx, |this, _| {
        this.select_host("p13-forward");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        available(&this.terminal)
    }) {
        assert!(Instant::now() < deadline, "P13 peer startup");
        std::thread::sleep(Duration::from_millis(5));
    }
    receipts
}

/// Terminal input events the receiving peer saw before `label`'s barrier.
fn barrier(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    receipts: &Receiver<ClientMessage>,
    label: &str,
) -> Vec<ClientPaneInputEvent> {
    view.update(cx, |this, _| {
        this.terminal
            .link
            .handle
            .as_ref()
            .unwrap()
            .request(
                "boot-v1",
                sheprd_client::Method::PaneScroll,
                serde_json::json!({"pane_id":"w1:p1","offset_from_bottom":100,"p13_barrier":label}),
            )
            .unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut events = Vec::new();
    loop {
        match receipts
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            ClientMessage::ClientShellPaneInput {
                pane_id,
                events: next,
            } => {
                assert_eq!(pane_id, "w1:p1", "forwarded input reached a wrong pane");
                events.extend(next);
            }
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                if request["params"]["p13_barrier"] == label {
                    return events;
                }
            }
            _ => {}
        }
    }
}

/// A barrier returns when its request reaches the peer, before the peer's
/// scroll surface and reply. `open_find` gives up when the connection reader
/// holds the inbox (`find_view.rs` try_lock), so let those replies land and
/// drain them before opening search again.
fn settle(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(2);
    std::thread::sleep(Duration::from_millis(200));
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        available(&this.terminal) && this.terminal.link.inbox.try_lock().is_ok()
    }) {
        assert!(Instant::now() < deadline, "P13 peer did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The handler GPUI keeps on the platform window for this field.
fn platform(field: &Entity<Field>) -> ElementInputHandler<Field> {
    ElementInputHandler::new(Bounds::default(), field.clone())
}

/// Opens search, closes it, and returns the retired field. The fence stays
/// paused; the next draw has not happened yet.
fn close(view: &Entity<Smoke>, window: &mut Window, cx: &mut gpui::App) -> Entity<Field> {
    view.update(cx, |this, cx| {
        this.terminal_focus.focus(window, cx);
        this.open_find(window, cx);
        let field = match &this.find {
            Some(bar) => bar.field.clone(),
            None => panic!(
                "search did not open: available={} inbox_free={} paused={}",
                available(&this.terminal),
                this.terminal.link.inbox.try_lock().is_ok(),
                this.find_handoff.paused
            ),
        };
        this.close_find(window, cx);
        assert!(this.find.is_none() && this.find_handoff.paused);
        // A draw since startup re-sent the window size; without the peer's
        // resized surface, production readiness holds fresh input.
        this.terminal.size = None;
        field
    })
}

fn acknowledge(view: &Entity<Smoke>, window: &mut Window, cx: &mut gpui::App) {
    view.update(cx, |this, _| {
        let ticket = this.begin_find_retirement(40).unwrap();
        assert!(this.complete_find_retirement(&ticket, 40, 41, window));
        assert!(!this.find_handoff.paused, "fence survived the retirement");
    });
}

#[gpui::test]
fn retired_field_forwards_to_the_terminal_only_until_the_first_post_close_draw(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let receipts = install(&view, cx);
    let mut frame = 0;
    let field = cx.update(|window, cx| {
        window.activate_window();
        let field = close(&view, window, cx);
        let mut handler = platform(&field);

        // Fence still paused: refused, and the field's inert reply stands.
        handler.replace_and_mark_text_in_range(None, "paused", None, window, cx);
        handler.replace_text_in_range(None, "paused", window, cx);
        assert_eq!(handler.marked_text_range(window, cx), None);
        assert!(!handler.accepts_text_input(window, cx));

        acknowledge(&view, window, cx);
        frame = view.read(cx).find_handoff.frames;
        // Fence open, no draw yet: the live client's calls reach the terminal.
        assert!(handler.accepts_text_input(window, cx));
        handler.replace_and_mark_text_in_range(None, "fwd", None, window, cx);
        assert_eq!(
            handler.marked_text_range(window, cx),
            Some(0..3),
            "query missed the terminal composition"
        );
        assert_eq!(
            view.read(cx).terminal.input.composition.text,
            "fwd",
            "mark missed the terminal composition"
        );
        handler.replace_text_in_range(None, "fwd", window, cx);
        field
    });
    assert_eq!(
        barrier(&view, cx, &receipts, "forwarded"),
        vec![ClientPaneInputEvent::TextCommit("fwd".into())],
        "forwarded commit did not reach the current target exactly once"
    );

    // The update above ended with a draw: the field is no longer the platform
    // handler, so its late callbacks are dropped.
    cx.update(|window, cx| {
        assert_ne!(
            view.read(cx).find_handoff.frames,
            frame,
            "no post-close draw"
        );
        let mut handler = platform(&field);
        assert!(!handler.accepts_text_input(window, cx));
        handler.replace_and_mark_text_in_range(None, "late", None, window, cx);
        handler.replace_text_in_range(None, "late", window, cx);
        assert_eq!(handler.marked_text_range(window, cx), None);
    });
    assert!(
        barrier(&view, cx, &receipts, "after-draw").is_empty(),
        "a callback after the swap reached the terminal"
    );

    // Reopened search, same frame and an open fence: the old field must reach
    // neither the terminal nor the new search field.
    settle(&view, cx);
    cx.update(|window, cx| {
        let old = close(&view, window, cx);
        acknowledge(&view, window, cx);
        let current = view.update(cx, |this, cx| {
            this.open_find(window, cx);
            this.find.as_ref().unwrap().field.clone()
        });
        let mut handler = platform(&old);
        assert!(!handler.accepts_text_input(window, cx));
        handler.replace_and_mark_text_in_range(None, "reopened", None, window, cx);
        handler.replace_text_in_range(None, "reopened", window, cx);
        assert!(
            current.read(cx).input.text.is_empty(),
            "old field reached new search"
        );
        assert!(view.read(cx).terminal.input.composition.text.is_empty());
        view.update(cx, |this, cx| this.close_find(window, cx));
    });
    assert!(
        barrier(&view, cx, &receipts, "reopened").is_empty(),
        "a callback during reopened search reached the terminal"
    );
}
