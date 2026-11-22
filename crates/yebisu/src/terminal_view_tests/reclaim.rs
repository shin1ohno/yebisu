use super::{
    GeometryMessage, ReclaimFixture, draw_terminal, draw_terminal_at_height, reclaim_fixture,
};
use crate::{
    board::{Navigation, Target},
    input_geometry::InputGeometry,
};
use gpui::{TestAppContext, VisualTestContext, px};
use sheprd_client::{
    ClientHandle, ConnectOptions, Error,
    protocol::{
        ClientClipboardImageTarget, ClientMessage, ClientPaneInputEvent, ClientSurfaceSize,
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const SHORT_CANVAS_HEIGHT: f32 = 400.;
const BARRIER: &str = "reclaim-test-barrier";

#[gpui::test]
fn local_resize_waits_for_a_new_frame_before_reclaiming(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);

    draw_terminal_at_height(&case.view, cx, SHORT_CANVAS_HEIGHT);
    let size = case.options(cx).surface_size;
    assert!(size.rows < case.sent.rows, "canvas did not shrink");
    assert_eq!(case.received_geometry(cx), [GeometryMessage::Resize(size)]);

    draw_terminal_at_height(&case.view, cx, SHORT_CANVAS_HEIGHT);
    assert_eq!(case.received_geometry(cx), []);

    // The same dimensions on a newer frame are now evidence of a mismatch.
    case.set_frame(cx, 0, 0);
    draw_terminal_at_height(&case.view, cx, SHORT_CANVAS_HEIGHT);
    case.assert_received_reclaim(cx, size);
}

#[gpui::test]
fn full_queue_retries_focus_without_resending_resize(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    let (handle, boot, pane) = case.connection(cx);
    // A pending clipboard preparation holds the writer at one FIFO entry.
    let reservation = handle
        .reserve_clipboard_input(&boot, ClientClipboardImageTarget::Pane(pane))
        .unwrap_or_else(|e| panic!("reserve writer: {e}"));
    // Leave exactly one slot after the pinned client's worker takes the reservation.
    const FILLERS: usize = 63; // sheprd-client's COMMAND_CAPACITY is 64.
    for _ in 0..FILLERS {
        handle
            .set_focus(&boot, true)
            .unwrap_or_else(|e| panic!("fill queue: {e}"));
    }
    case.set_frame(cx, 0, 1);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        draw_terminal(&case.view, cx);
        if case
            .view
            .read_with(cx, |this, _| this.terminal.surface_reclaim.is_some())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "resize did not take the last slot"
        );
        std::thread::yield_now();
    }
    assert!(matches!(handle.set_focus(&boot, false), Err(Error::Full)));
    draw_terminal(&case.view, cx); // The queue is still full on the next paint.

    drop(reservation);
    let mut expected = vec![GeometryMessage::Focus(true); FILLERS];
    expected.push(GeometryMessage::Resize(case.sent));
    assert_eq!(case.received_geometry(cx), expected);

    draw_terminal(&case.view, cx);
    assert_eq!(
        case.received_geometry(cx),
        [GeometryMessage::Focus(false), GeometryMessage::Focus(true)]
    );
}

#[gpui::test]
fn reconnect_suspension_defers_reclaim_until_resumed(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.view.update(cx, |this, _| {
        this.terminal.reconnect.suspended = true;
    });
    case.set_frame(cx, 0, 1);
    draw_terminal(&case.view, cx);
    assert_eq!(case.received_geometry(cx), []);

    case.view.update(cx, |this, _| {
        this.terminal.reconnect.suspended = false;
    });
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);
}

#[gpui::test]
fn navigation_defers_reclaim_until_the_transition_ends(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    let (_, boot, pane) = case.connection(cx);
    case.view.update(cx, |this, _| {
        this.terminal.navigating = Some(Navigation {
            boot,
            revision: 1,
            target: Target::Agent(pane),
            request: "reclaim-navigation".into(),
        });
    });
    case.set_frame(cx, 0, 1);
    draw_terminal(&case.view, cx);
    assert_eq!(case.received_geometry(cx), []);

    case.view.update(cx, |this, _| {
        this.terminal.navigating = None;
    });
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);
}

#[gpui::test]
fn a_claim_on_the_old_tab_does_not_suppress_reclaim_on_the_new_tab(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.set_frame(cx, 0, 1);
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);

    case.view.update(cx, |this, _| {
        let snapshot = this
            .terminal
            .live
            .snapshot
            .as_mut()
            .unwrap_or_else(|| panic!("no snapshot"));
        Arc::make_mut(snapshot).focused_tab_id = Some("second-tab".into());
    });
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);
}

#[gpui::test]
fn a_claim_from_an_old_boot_does_not_suppress_reclaim_on_the_current_boot(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    let (_, boot, _) = case.connection(cx);
    case.set_frame(cx, 0, 1);
    case.set_boot_and_reported_focus(cx, "previous-boot");
    draw_terminal(&case.view, cx);
    assert!(
        case.view
            .read_with(cx, |this, _| this.terminal.surface_reclaim.is_some())
    );

    // The peer accepts the current boot only. Start already focused on it so the
    // next paint exercises claim identity rather than the initial focus report.
    case.set_boot_and_reported_focus(cx, &boot);
    assert_eq!(case.received_geometry(cx), []);
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);
}

#[gpui::test]
fn a_claim_at_the_old_size_does_not_suppress_reclaim_at_the_current_size(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.set_frame(cx, 0, 1);
    draw_terminal(&case.view, cx);
    case.assert_received_reclaim(cx, case.sent);

    let size = case.view.update(cx, |this, _| {
        let terminal = &mut this.terminal;
        let mut bounds = terminal.geometry.canvas_bounds();
        bounds.size.height = px(SHORT_CANVAS_HEIGHT);
        let geometry = InputGeometry::from_canvas(
            bounds,
            terminal.geometry.cell_width,
            terminal.geometry.cell_height,
        );
        let size = crate::terminal::viewport(
            f32::from(geometry.grid.size.width),
            f32::from(geometry.grid.size.height),
            geometry.cell_width,
            geometry.cell_height,
        );
        let options = terminal.size.unwrap_or_else(|| panic!("no measured size"));
        terminal.resize_if_ready(ConnectOptions {
            surface_size: size,
            ..options
        });
        size
    });
    assert!(size.rows < case.sent.rows);
    assert_eq!(case.received_geometry(cx), [GeometryMessage::Resize(size)]);

    // The size is already queued before paint. Keep the old claim and the
    // mismatched frame so only the requested-size comparison can rearm it.
    draw_terminal_at_height(&case.view, cx, SHORT_CANVAS_HEIGHT);
    case.assert_received_reclaim(cx, size);
}

impl ReclaimFixture {
    fn connection(&self, cx: &mut VisualTestContext) -> (ClientHandle, String, String) {
        self.view.read_with(cx, |this, _| {
            let terminal = &this.terminal;
            let handle = terminal
                .link
                .handle
                .clone()
                .unwrap_or_else(|| panic!("no handle"));
            let snapshot = terminal
                .live
                .snapshot
                .as_ref()
                .unwrap_or_else(|| panic!("no snapshot"));
            let pane = snapshot
                .focused_pane_id
                .clone()
                .unwrap_or_else(|| panic!("no focused pane"));
            (handle, snapshot.boot_id.clone(), pane)
        })
    }

    fn options(&self, cx: &mut VisualTestContext) -> ConnectOptions {
        self.view.read_with(cx, |this, _| {
            this.terminal
                .size
                .unwrap_or_else(|| panic!("no measured size"))
        })
    }

    fn set_boot_and_reported_focus(&self, cx: &mut VisualTestContext, boot: &str) {
        self.view.update(cx, |this, _| {
            let terminal = &mut this.terminal;
            let snapshot = terminal
                .live
                .snapshot
                .as_mut()
                .unwrap_or_else(|| panic!("no snapshot"));
            let surface = terminal
                .live
                .surface
                .as_mut()
                .unwrap_or_else(|| panic!("no surface"));
            Arc::make_mut(snapshot).boot_id = boot.to_owned();
            Arc::make_mut(surface).boot_id = boot.to_owned();
            terminal.surface_focus = Some((terminal.link.generation, boot.to_owned(), true));
        });
    }

    // A paste sent after paint fences the writer's FIFO. Use its receipt to
    // prove that even an empty list includes every geometry message from paint.
    fn received_geometry(&self, cx: &mut VisualTestContext) -> Vec<GeometryMessage> {
        let (handle, boot, pane) = self.connection(cx);
        let deadline = Instant::now() + Duration::from_secs(5);
        let barrier = ClientPaneInputEvent::Paste(BARRIER.into());
        loop {
            match handle.send_input(&boot, &pane, [barrier.clone()]) {
                Ok(()) => break,
                Err(Error::Full) if Instant::now() < deadline => std::thread::yield_now(),
                Err(e) => panic!("queue peer barrier: {e}"),
            }
        }

        let mut messages = Vec::new();
        loop {
            let message = self
                .receipts
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| {
                    panic!("peer did not receive barrier: {e}; geometry: {messages:?}")
                });
            match message {
                ClientMessage::ClientShellResize { surface_size, .. } => {
                    messages.push(GeometryMessage::Resize(surface_size));
                }
                ClientMessage::ClientShellFocus { focused } => {
                    messages.push(GeometryMessage::Focus(focused));
                }
                ClientMessage::ClientShellPaneInput { events, .. }
                    if events == [barrier.clone()] =>
                {
                    return messages;
                }
                _ => {}
            }
        }
    }

    fn assert_received_reclaim(&self, cx: &mut VisualTestContext, size: ClientSurfaceSize) {
        assert_eq!(
            self.received_geometry(cx),
            [
                GeometryMessage::Resize(size),
                GeometryMessage::Focus(false),
                GeometryMessage::Focus(true)
            ]
        );
    }
}
