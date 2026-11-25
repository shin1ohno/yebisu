use super::CursorWatch;
use gpui::{Bounds, point, px, size};

fn cell(column: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(
        point(px(10.0 + 9.0 * column), px(40.0)),
        size(px(9.0), px(18.0)),
    )
}

#[test]
fn first_frame_reports() {
    let mut watch = CursorWatch::default();
    assert!(watch.moved(cell(0.0)));
}

#[test]
fn same_cell_does_not_report() {
    let mut watch = CursorWatch::default();
    assert!(watch.moved(cell(0.0)));
    assert!(
        !watch.moved(cell(0.0)),
        "a same-cell repaint reported a move"
    );
}

#[test]
fn moved_cell_reports_once() {
    let mut watch = CursorWatch::default();
    let moves: Vec<bool> = [cell(0.0), cell(1.0), cell(1.0)]
        .into_iter()
        .map(|c| watch.moved(c))
        .collect();
    assert_eq!(moves, [true, true, false]);
}

#[test]
fn returning_to_a_previous_cell_reports() {
    let mut watch = CursorWatch::default();
    assert!(watch.moved(cell(0.0)));
    assert!(watch.moved(cell(1.0)));
    assert!(watch.moved(cell(0.0)));
}

#[test]
fn reports_are_counted() {
    let mut watch = CursorWatch::default();
    assert_eq!(watch.invalidations(), 0);
    watch.reported();
    watch.reported();
    assert_eq!(watch.invalidations(), 2);
}

/// A connected Herdr 1 terminal (the quiet wire peer), drawn at least once.
fn connected(
    cx: &mut gpui::TestAppContext,
) -> (
    gpui::Entity<crate::app::Smoke>,
    &mut gpui::VisualTestContext,
    std::sync::mpsc::Receiver<sheprd_client::protocol::ClientMessage>,
) {
    use std::time::{Duration, Instant};
    let (view, cx) = cx.add_window_view(|_, cx| crate::app::Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, wire) = crate::mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.handoff.stamp.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "terminal frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx, wire)
}

fn drawn(view: &gpui::Entity<crate::app::Smoke>, cx: &mut gpui::VisualTestContext) -> bool {
    view.update(cx, |this, _| {
        this.terminal.canvas_bounds.is_some() && this.terminal.live.surface.is_some()
    })
}

fn invalidations(view: &gpui::Entity<crate::app::Smoke>, cx: &mut gpui::VisualTestContext) -> u64 {
    view.update(cx, |this, _| this.terminal.input_indicator.invalidations())
}

/// Draws the terminal again, with the surface `change` leaves.
fn redraw(
    view: &gpui::Entity<crate::app::Smoke>,
    cx: &mut gpui::VisualTestContext,
    change: impl FnOnce(&mut sheprd_client::protocol::PaneSurfaceFrame),
) {
    view.update(cx, |this, cx| {
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        let mut surface = (**shown).clone();
        change(&mut surface);
        this.terminal.live.surface = Some(std::sync::Arc::new(surface));
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn repaint_without_move_does_not_invalidate(cx: &mut gpui::TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    assert!(drawn(&view, cx), "precondition: the terminal was drawn");
    let before = invalidations(&view, cx);
    redraw(&view, cx, |_| {});
    redraw(&view, cx, |_| {});
    assert_eq!(
        invalidations(&view, cx),
        before,
        "a same-cell repaint invalidated"
    );
}

#[gpui::test]
fn frame_with_moved_cursor_invalidates_once(cx: &mut gpui::TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    assert!(drawn(&view, cx), "precondition: the terminal was drawn");
    let before = invalidations(&view, cx);
    redraw(&view, cx, |surface| {
        let Some(cursor) = surface.frame.cursor.as_mut() else {
            panic!("fixture cursor");
        };
        cursor.x += 1;
    });
    assert_eq!(invalidations(&view, cx), before + 1, "a one-cell move");
    redraw(&view, cx, |_| {});
    assert_eq!(
        invalidations(&view, cx),
        before + 1,
        "the moved cell repainted"
    );
}

#[gpui::test]
fn move_while_settings_own_text_is_reported_after(cx: &mut gpui::TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    assert!(drawn(&view, cx), "precondition: the terminal was drawn");
    let before = invalidations(&view, cx);
    view.update(cx, |this, _| this.settings_modal = true);
    redraw(&view, cx, |surface| {
        let Some(cursor) = surface.frame.cursor.as_mut() else {
            panic!("fixture cursor");
        };
        cursor.x += 1;
    });
    assert_eq!(
        invalidations(&view, cx),
        before,
        "reported while bounds_for_range cannot answer"
    );
    view.update(cx, |this, _| this.settings_modal = false);
    redraw(&view, cx, |_| {});
    assert_eq!(
        invalidations(&view, cx),
        before + 1,
        "the move was not reported once text came back"
    );
}
