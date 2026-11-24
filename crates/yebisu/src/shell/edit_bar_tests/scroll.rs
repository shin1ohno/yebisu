//! Issue #137 PR 2: the edit bar moves sideways under a sideways swipe only.
use super::support::{selection, window};
use crate::mock_peer;
use gpui::{
    Modifiers, ScrollDelta, ScrollEvent, ScrollSource, TouchPhase, VisualTestContext, point, px,
    size,
};
use std::time::{Duration, Instant};

fn swipe(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, x: f32, y: f32) {
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(x), px(y))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Started,
        source: ScrollSource::Touch,
    });
    cx.run_until_parked();
}

/// A narrow window, so the bar is wider than its room and scrolls.
#[gpui::test]
fn the_overflowing_edit_bar_ignores_vertical_swipes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(360.), px(800.)));
    let (client, _) = mock_peer::connect_quiet().unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host("edit-bar-scroll");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < deadline, "no frame");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, cx| {
        this.handoff.selection = Some(selection());
        cx.notify();
    });
    cx.run_until_parked();
    let copy = |cx: &mut VisualTestContext| {
        cx.debug_bounds("edit-bar-copy")
            .unwrap_or_else(|| panic!("the edit bar's Copy"))
    };
    let at = copy(cx).center();
    let start = copy(cx).left();
    swipe(cx, at, 0., -80.);
    assert_eq!(copy(cx).left(), start, "a vertical swipe moved the bar");
    swipe(cx, at, -80., 0.);
    assert!(
        copy(cx).left() < start,
        "a sideways swipe must move the overflowing bar"
    );
}
