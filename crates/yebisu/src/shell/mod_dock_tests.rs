//! R4: the frame draws the adopted scene at L0's M2 rectangle, adopts off
//! the UI thread, and routes a tap there back as an event target.
use crate::{
    app::Smoke,
    layout::{M2_RANGE, ModSlot, Rect},
    mock_peer,
    mod_view::{EventKind, Fixture},
};
use gpui::{Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, point, px, size};
use std::time::{Duration, Instant};

/// A Herdr 1 terminal on the private peer, drawn in the frame at 1376x1032.
fn terminal(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (client, _receipts) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, _| {
        this.select_host("mod-dock");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx)
}

fn redraw(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

fn m2(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<Rect> {
    cx.update(|window, cx| view.update(cx, |this, _| this.frame_layout(window).m2))
}

fn as_bounds(rect: Rect) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(rect.x), px(rect.y)),
        size: size(px(rect.width), px(rect.height)),
    }
}

#[gpui::test]
fn r4_the_scene_fills_l0s_m2_rectangle(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    assert_eq!(m2(&view, cx), None);
    assert!(cx.debug_bounds("shell-mod-dock").is_none());
    view.update(cx, |this, _| {
        this.install_mod_scene(Fixture::Counter.adopt())
    });
    redraw(&view, cx);
    let rect = m2(&view, cx).unwrap_or_else(|| panic!("M2 withdrawn at 1376 pt"));
    assert_eq!(rect.width, M2_RANGE.default);
    let expected = as_bounds(rect);
    assert_eq!(cx.debug_bounds("shell-mod-dock"), Some(expected));
    assert_eq!(cx.debug_bounds("mod-slot"), Some(expected));
    assert_eq!(cx.debug_bounds("mod-node-0"), Some(expected));
    let plus = cx
        .debug_bounds("mod-node-3")
        .unwrap_or_else(|| panic!("+1 not painted"));
    assert!(expected.contains(&plus.origin));
    cx.simulate_click(plus.center(), Modifiers::none());
    cx.run_until_parked();
    let events = view.update(cx, |this, _| this.shell.mod_view.events.drain());
    let routed: Vec<_> = events.iter().map(|e| (e.target, e.kind)).collect();
    assert_eq!(routed, [(3, EventKind::Click)]);
}

#[gpui::test]
fn r4_a_fixture_is_adopted_off_the_ui_thread_and_requests_m2(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, cx| this.load_mod_fixture("form".into(), cx));
    cx.run_until_parked();
    let (region, drawn) = view.update(cx, |this, _| {
        (
            this.shell.fit.mod_regions.m2,
            this.shell.mod_view.scene.is_some(),
        )
    });
    assert_eq!(region, ModSlot::Wants(M2_RANGE.default));
    assert!(drawn);
    redraw(&view, cx);
    assert!(cx.debug_bounds("mod-node-2").is_some());
}

#[gpui::test]
fn r4_a_refused_fixture_leaves_m2_empty(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, cx| this.load_mod_fixture("missing".into(), cx));
    cx.run_until_parked();
    redraw(&view, cx);
    let (region, error) = view.update(cx, |this, _| {
        (
            this.shell.fit.mod_regions.m2,
            this.shell.mod_view.error.clone(),
        )
    });
    assert_eq!(region, ModSlot::Empty);
    assert!(error.is_some());
    assert_eq!(m2(&view, cx), None);
    assert!(cx.debug_bounds("shell-mod-dock").is_none());
}

#[gpui::test]
fn r8_a_dropped_line_dims_m2_and_stops_its_input(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| {
        this.install_mod_scene(Fixture::Counter.adopt())
    });
    redraw(&view, cx);
    view.update(cx, |this, _| {
        // The status bar hands the line state to the clock on every draw.
        this.terminal.live.connected = false;
        this.shell.fit.disconnect.observe(false, Instant::now());
    });
    redraw(&view, cx);
    assert!(view.update(cx, |this, _| this.shell.mod_view.faded));
    let plus = cx
        .debug_bounds("mod-node-3")
        .unwrap_or_else(|| panic!("+1 not painted while dimmed"));
    cx.simulate_click(plus.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        view.update(cx, |this, _| this.shell.mod_view.events.len()),
        0
    );
}
