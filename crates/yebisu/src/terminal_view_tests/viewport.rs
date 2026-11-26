use crate::{app::Smoke, find, layout, mock_peer};
use gpui::{
    Bounds, ContentMask, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets,
    canvas, point, prelude::*, px, size,
};
use sheprd_client::Client;
use sheprd_client::protocol::{ClientMessage, ClientSurfaceSize};
use std::{
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};

#[path = "reclaim.rs"]
mod reclaim;

#[gpui::test]
fn resize_fits_the_painted_terminal_across_chrome_states(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, receipts) = find::peer::connect().unwrap_or_else(|e| panic!("Find peer: {e}"));
    install(&view, cx, client);
    let mut previous = None;
    for (width, height, find_open, rail, edit_bar) in [
        (1376., 1032., false, false, false),
        (1376., 1032., true, false, false),
        (1376., 1032., false, false, true),
        (1288., 966., false, true, false),
        (826., 622., true, true, false),
        (826., 622., false, true, true),
        (475., 452., false, true, false),
    ] {
        cx.simulate_resize(size(px(width), px(height)));
        let windowed = width < 1000.;
        cx.update(|window, app| {
            view.update(app, |this, cx| {
                this.shell.fit.insets = Some(WindowInsets {
                    safe_area: Edges {
                        top: px(28.),
                        bottom: px(16.),
                        ..Edges::default()
                    },
                    ..WindowInsets::default()
                });
                this.shell.fit.window_controls = windowed.then_some(Edges {
                    top: px(37.),
                    left: px(78.),
                    ..Edges::default()
                });
                if this.actions.board_hidden != rail {
                    this.toggle_sidebar();
                }
                if find_open {
                    this.open_find(window, cx);
                } else {
                    this.close_find(window, cx);
                }
                this.handoff.select_mode = edit_bar;
                cx.notify();
            });
        });
        cx.run_until_parked();
        let slot = painted(cx, "shell-terminal");
        let tabs = painted(cx, "shell-tab-row");
        let status_height = layout::STATUS_BAR_HEIGHT;
        // A window with controls gives its bottom safe area to the status bar
        // (`layout::bottom_band`); full screen keeps the 16 pt.
        let bottom = if windowed { 0. } else { 16. };
        assert!(slot.top() >= tabs.bottom());
        assert_eq!(
            slot.bottom(),
            px(height - bottom - status_height - layout::RULE)
        );
        assert_eq!(cx.debug_bounds("edit-bar").is_some(), edit_bar);
        assert_eq!(view.read_with(cx, |this, _| this.find.is_some()), find_open);

        let dimensions = view.read_with(cx, |this, _| this.terminal.size.map(|s| s.surface_size));
        if dimensions != previous {
            let dimensions = dimensions.unwrap_or_else(|| panic!("no size for {width}×{height}"));
            next_message(
                &receipts,
                |message| matches!(message, ClientMessage::ClientShellResize { surface_size, .. } if *surface_size == dimensions),
            );
            previous = Some(dimensions);
        }
        let sent = previous.unwrap_or_else(|| panic!("no resize for {width}×{height}"));
        assert_fits(&view, cx, sent, slot);
    }
}

#[gpui::test]
fn resize_uses_the_visible_part_of_a_clipped_canvas(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    let view = cx.new(Smoke::new);
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("private peer: {e}"));
    install(&view, cx, client);
    let origin = point(px(76.), px(81.));
    let clip = Bounds::new(origin, size(px(720.), px(396.)));
    let paint = view.update(cx, |this, cx| this.terminal_paint_callback(cx));
    cx.draw(origin, size(px(800.), px(460.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                    paint(bounds, (), window, cx);
                });
            },
        )
        .size_full()
    });
    let ClientMessage::ClientShellResize {
        surface_size: sent, ..
    } = next_message(&receipts, |message| {
        matches!(message, ClientMessage::ClientShellResize { .. })
    })
    else {
        unreachable!()
    };
    assert_fits(&view, cx, sent, clip);
}

#[gpui::test]
fn active_terminal_claims_geometry_after_sending_its_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1288.), px(966.)));
    cx.update(|window, _| window.activate_window());
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("private peer: {e}"));
    install(&view, cx, client);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(cx.update(|window, _| window.is_window_active()));

    let mut requested_size = None;
    next_message(&receipts, |message| {
        if let ClientMessage::ClientShellResize { surface_size, .. } = message {
            requested_size = Some(*surface_size);
        }
        matches!(message, ClientMessage::ClientShellFocus { focused: true })
    });
    let sized = view.read_with(cx, |this, _| this.terminal.size.map(|s| s.surface_size));
    assert!(requested_size.is_some());
    assert_eq!(requested_size, sized, "focus preceded the measured resize");
    view.update(cx, |this, cx| {
        this.terminal.open = false;
        this.poll_terminal(cx);
    });
    next_message(&receipts, |message| {
        matches!(message, ClientMessage::ClientShellFocus { focused: false })
    });
}

fn install(view: &Entity<Smoke>, cx: &mut VisualTestContext, client: Client) {
    view.update(cx, |this, _| {
        this.select_host("bottom-row");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && this.terminal.live.connected
    }) {
        assert!(Instant::now() < deadline, "private peer has no surface");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn painted(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"))
}

fn next_message(
    receipts: &Receiver<ClientMessage>,
    mut matches: impl FnMut(&ClientMessage) -> bool,
) -> ClientMessage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let message = receipts
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e| panic!("Herdr did not receive the expected message: {e}"));
        if matches(&message) {
            return message;
        }
    }
}

fn assert_fits(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    sent: ClientSurfaceSize,
    visible: Bounds<Pixels>,
) {
    let geometry = view.read_with(cx, |this, _| this.terminal.geometry);
    let edge = geometry.grid.origin
        + point(
            px(f32::from(sent.cols) * geometry.cell_width),
            px(f32::from(sent.rows) * geometry.cell_height),
        );
    assert!(geometry.grid.left() >= visible.left());
    assert!(geometry.grid.top() >= visible.top());
    assert!(
        edge.y <= visible.bottom() + px(0.01),
        "{sent:?} ends at {edge:?}, below the visible terminal {visible:?}"
    );
    assert!(
        edge.x <= visible.right() + px(0.01),
        "{sent:?} ends at {edge:?}, right of the visible terminal {visible:?}"
    );
}

#[gpui::test]
fn terminal_mismatch_resends_resize_before_reclaiming_focus(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.draw_frame(cx, 1, 0);
    case.assert_reclaim();
}

#[gpui::test]
fn clipped_pty_rows_trigger_reclaim_even_with_matching_frame_dimensions(cx: &mut TestAppContext) {
    use sheprd_client::protocol::{PaneSurfacePane, PaneSurfaceScrollMetrics, SurfaceRect};

    let (case, cx) = reclaim_fixture(cx);
    case.view.update(cx, |this, _| {
        let surface = this
            .terminal
            .live
            .surface
            .as_mut()
            .unwrap_or_else(|| panic!("no frame"));
        let surface = Arc::make_mut(surface);
        let rect = SurfaceRect {
            x: 0,
            y: 0,
            width: case.sent.cols,
            height: case.sent.rows,
        };
        surface.surface_revision += 1;
        surface.panes.push(PaneSurfacePane {
            pane_id: "w1:p1".into(),
            content_revision: 2,
            rect,
            inner_rect: rect,
            scrollbar_rect: None,
            scroll: Some(PaneSurfaceScrollMetrics {
                offset_from_bottom: 0,
                max_offset_from_bottom: 0,
                viewport_rows: u64::from(rect.height) + 9,
            }),
            focused: true,
            mouse_reporting: false,
            sgr_pixel_mouse: false,
            alternate_screen_active: false,
            pixel_width: 0,
            pixel_height: 0,
        });
    });
    draw_terminal(&case.view, cx);
    case.assert_reclaim();
}

#[gpui::test]
fn another_mismatched_frame_does_not_reclaim_twice(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.draw_frame(cx, 0, 1);
    case.assert_reclaim();
    case.draw_frame(cx, 0, 1);
    assert!(case.geometry_messages().is_empty());
}

#[gpui::test]
fn matching_frame_rearms_geometry_reclaim(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    case.draw_frame(cx, 1, 0);
    case.assert_reclaim();
    case.draw_frame(cx, 0, 0);
    assert!(case.geometry_messages().is_empty());
    case.draw_frame(cx, 1, 0);
    case.assert_reclaim();
}

#[gpui::test]
fn background_terminal_does_not_reclaim_geometry(cx: &mut TestAppContext) {
    let (case, cx) = reclaim_fixture(cx);
    cx.update(|_, app| {
        app.open_window(gpui::WindowOptions::default(), |window, app| {
            window.activate_window();
            app.new(|_| gpui::Empty)
        })
        .unwrap_or_else(|e| panic!("second window: {e}"));
    });
    cx.run_until_parked();
    assert!(!cx.update(|window, _| window.is_window_active()));
    case.draw_frame(cx, 0, 1);
    assert_eq!(case.geometry_messages(), [GeometryMessage::Focus(false)]);

    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    case.draw_frame(cx, 0, 0);
    assert_eq!(case.geometry_messages(), [GeometryMessage::Focus(true)]);
    case.draw_frame(cx, 0, 1);
    case.assert_reclaim();
}

#[derive(Clone, Debug, PartialEq)]
enum GeometryMessage {
    Resize(ClientSurfaceSize),
    Focus(bool),
}

struct ReclaimFixture {
    view: Entity<Smoke>,
    receipts: Receiver<ClientMessage>,
    sent: ClientSurfaceSize,
}

fn reclaim_fixture(cx: &mut TestAppContext) -> (ReclaimFixture, &mut VisualTestContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(cx.update(|window, _| window.is_window_active()));
    let view = cx.new(Smoke::new);
    let (client, receipts) =
        mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("private peer: {e}"));
    install(&view, cx, client);
    draw_terminal(&view, cx);
    let sent = view.read_with(cx, |this, _| {
        this.terminal
            .size
            .unwrap_or_else(|| panic!("no measured size"))
            .surface_size
    });
    let case = ReclaimFixture {
        view,
        receipts,
        sent,
    };
    assert_eq!(
        case.geometry_messages(),
        [GeometryMessage::Resize(sent), GeometryMessage::Focus(true)]
    );
    case.draw_frame(cx, 0, 0);
    assert!(case.geometry_messages().is_empty());
    (case, cx)
}

fn draw_terminal(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    draw_terminal_at_height(view, cx, 600.);
}

fn draw_terminal_at_height(view: &Entity<Smoke>, cx: &mut VisualTestContext, height: f32) {
    let paint = view.update(cx, |this, cx| this.terminal_paint_callback(cx));
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(height)), |_, _| {
        canvas(|_, _, _| (), paint).size_full()
    });
}

impl ReclaimFixture {
    fn draw_frame(&self, cx: &mut VisualTestContext, fewer_cols: u16, fewer_rows: u16) {
        self.set_frame(cx, fewer_cols, fewer_rows);
        draw_terminal(&self.view, cx);
    }

    fn set_frame(&self, cx: &mut VisualTestContext, fewer_cols: u16, fewer_rows: u16) {
        self.view.update(cx, |this, _| {
            let surface = this
                .terminal
                .live
                .surface
                .as_mut()
                .unwrap_or_else(|| panic!("no frame"));
            let surface = Arc::make_mut(surface);
            surface.surface_revision += 1;
            surface.frame.width = self.sent.cols - fewer_cols;
            surface.frame.height = self.sent.rows - fewer_rows;
            let blank = surface
                .frame
                .cells
                .last()
                .cloned()
                .unwrap_or_else(|| panic!("no fixture cells"));
            surface.frame.cells.resize(
                usize::from(surface.frame.width) * usize::from(surface.frame.height),
                blank,
            );
        });
    }

    fn geometry_messages(&self) -> Vec<GeometryMessage> {
        let mut messages = Vec::new();
        while let Ok(message) = self.receipts.recv_timeout(Duration::from_millis(150)) {
            match message {
                ClientMessage::ClientShellResize { surface_size, .. } => {
                    messages.push(GeometryMessage::Resize(surface_size));
                }
                ClientMessage::ClientShellFocus { focused } => {
                    messages.push(GeometryMessage::Focus(focused));
                }
                _ => {}
            }
        }
        messages
    }

    fn assert_reclaim(&self) {
        assert_eq!(
            self.geometry_messages(),
            [
                GeometryMessage::Resize(self.sent),
                GeometryMessage::Focus(false),
                GeometryMessage::Focus(true)
            ],
        );
    }
}
