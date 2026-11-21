use crate::{app::Smoke, find, layout, mock_peer};
use gpui::{
    Bounds, ContentMask, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets,
    canvas, point, prelude::*, px, size,
};
use sheprd_client::Client;
use sheprd_client::protocol::{ClientMessage, ClientSurfaceSize};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

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
        assert!(slot.top() >= tabs.bottom());
        assert_eq!(
            slot.bottom(),
            px(height - 16. - status_height - layout::RULE)
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
