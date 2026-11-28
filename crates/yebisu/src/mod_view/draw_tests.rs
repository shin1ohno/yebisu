//! R3 and R5 to R8 drawn with GPUI: element frames inside a 320 x 600 region
//! at (100, 50), and the event targets taps and keys produce.
use super::*;
use gpui::{
    Bounds, Context, Entity, Modifiers, Pixels, Render, TestAppContext, VisualTestContext, Window,
    div, prelude::*, px, size,
};
use std::rc::Rc;

const SLOT_X: f32 = 100.;
const SLOT_Y: f32 = 50.;
const SLOT_W: f32 = 320.;
const SLOT_H: f32 = 600.;

/// Draws one region at the slot rectangle, as the shell does for M2.
pub(super) struct Host {
    pub(super) state: ModViewState,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self
            .state
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let weak = cx.entity().downgrade();
        let route: Route = Rc::new(move |hit, _, cx| {
            weak.update(cx, |this, cx| {
                this.state.hit(hit);
                cx.notify();
            })
            .ok();
        });
        let region = self
            .state
            .scene
            .clone()
            .map(|scene| slot(&scene, self.state.faded, &focus, route));
        div().size_full().relative().child(
            div()
                .absolute()
                .left(px(SLOT_X))
                .top(px(SLOT_Y))
                .w(px(SLOT_W))
                .h(px(SLOT_H))
                .children(region),
        )
    }
}

pub(super) fn host(
    cx: &mut TestAppContext,
    fixture: Fixture,
) -> (Entity<Host>, &mut VisualTestContext) {
    let scene = fixture
        .adopt()
        .unwrap_or_else(|e| panic!("{}: {e}", fixture.name()));
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut state = ModViewState::default();
        state.install(Ok(scene));
        Host { state }
    });
    cx.simulate_resize(size(px(800.), px(800.)));
    cx.run_until_parked();
    (view, cx)
}

pub(super) fn bounds(cx: &mut VisualTestContext, index: usize) -> Bounds<Pixels> {
    // debug_bounds takes a 'static selector; tests name a handful of nodes.
    let name: &'static str = Box::leak(format!("mod-node-{index}").into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not painted"))
}

/// x, y, width, height relative to the region's origin.
fn frame(cx: &mut VisualTestContext, index: usize) -> (f32, f32, f32, f32) {
    let b = bounds(cx, index);
    (
        f32::from(b.origin.x) - SLOT_X,
        f32::from(b.origin.y) - SLOT_Y,
        f32::from(b.size.width),
        f32::from(b.size.height),
    )
}

fn tap(cx: &mut VisualTestContext, index: usize) {
    let at = bounds(cx, index).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn events(view: &Entity<Host>, cx: &mut VisualTestContext) -> Vec<(usize, u64, EventKind)> {
    view.update(cx, |host, _| {
        host.state
            .events
            .drain()
            .into_iter()
            .map(|e| (e.target, e.callback, e.kind))
            .collect()
    })
}

#[gpui::test]
fn r3_counter_column_stacks_with_padding_and_gap(cx: &mut TestAppContext) {
    let (_view, cx) = host(cx, Fixture::Counter);
    assert_eq!(frame(cx, 0), (0., 0., SLOT_W, SLOT_H));
    assert_eq!(frame(cx, 1), (12., 12., SLOT_W - 24., 24.));
    assert_eq!(frame(cx, 2), (12., 44., SLOT_W - 24., 44.));
    assert_eq!(frame(cx, 3), (12., 44., 96., 44.));
    assert_eq!(frame(cx, 4), (116., 44., 96., 44.));
}

#[gpui::test]
fn r3_status_row_places_cells_left_to_right_centred(cx: &mut TestAppContext) {
    let (_view, cx) = host(cx, Fixture::Status);
    assert_eq!(frame(cx, 0), (0., 0., SLOT_W, SLOT_H));
    let y = (SLOT_H - 28.) / 2.;
    assert_eq!(frame(cx, 1), (4., y, 80., 28.));
    assert_eq!(frame(cx, 2), (88., y, 80., 28.));
    assert_eq!(frame(cx, 3), (172., y, 80., 28.));
}

#[gpui::test]
fn r3_form_field_takes_the_full_width(cx: &mut TestAppContext) {
    let (_view, cx) = host(cx, Fixture::Form);
    assert_eq!(frame(cx, 1), (8., 8., SLOT_W - 16., 20.));
    assert_eq!(frame(cx, 2), (8., 34., SLOT_W - 16., 36.));
    assert_eq!(frame(cx, 3).1, 34.);
    assert_eq!(frame(cx, 4), (8., 76., 80., 36.));
}

#[gpui::test]
fn r3_nested_padding_accumulates(cx: &mut TestAppContext) {
    let (_view, cx) = host(cx, Fixture::Nested);
    assert_eq!(frame(cx, 1), (16., 16., SLOT_W - 32., 48.));
    assert_eq!(frame(cx, 2), (24., 24., 64., 32.));
}

#[gpui::test]
fn r5_taps_report_their_node_index_in_tap_order(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Counter);
    tap(cx, 4);
    tap(cx, 3);
    tap(cx, 3);
    assert_eq!(
        events(&view, cx),
        [
            (4, 8, EventKind::Click),
            (3, 7, EventKind::Click),
            (3, 7, EventKind::Click)
        ]
    );
}

#[gpui::test]
fn r5_a_tap_inside_a_callback_node_targets_that_node(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Form);
    // Node 3 is the text inside the field (node 2); only node 2 has callbacks.
    tap(cx, 3);
    assert_eq!(events(&view, cx), [(2, 11, EventKind::Click)]);
}

#[gpui::test]
fn r6_taps_outside_every_callback_node_queue_nothing(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Status);
    for index in 0..4 {
        tap(cx, index);
    }
    assert!(events(&view, cx).is_empty());
}

#[gpui::test]
fn r6_a_tap_on_the_counters_label_queues_nothing(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Counter);
    tap(cx, 1);
    tap(cx, 0);
    assert!(events(&view, cx).is_empty());
}

#[gpui::test]
fn r5_the_innermost_callback_node_takes_the_tap_alone(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Nested);
    // The button (node 2, click 21) sits in a tappable box (node 1, click 20).
    tap(cx, 2);
    assert_eq!(events(&view, cx), [(2, 21, EventKind::Click)]);
    let inner = bounds(cx, 1);
    let beside = gpui::point(inner.right() - px(4.), inner.center().y);
    cx.simulate_click(beside, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(events(&view, cx), [(1, 20, EventKind::Click)]);
    // The outer box (node 0) has no callback: its padding queues nothing.
    let outer = bounds(cx, 0);
    cx.simulate_click(
        outer.origin + gpui::point(px(4.), px(4.)),
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert!(events(&view, cx).is_empty());
}

#[gpui::test]
fn r7_keys_reach_the_tapped_field_and_not_before(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Form);
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(events(&view, cx).is_empty());
    tap(cx, 2);
    cx.simulate_keystrokes("b c");
    cx.run_until_parked();
    let keys = view.update(cx, |host, _| {
        host.state
            .events
            .drain()
            .into_iter()
            .map(|e| (e.target, e.callback, e.kind, e.key))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        keys,
        [
            (2, 11, EventKind::Click, None),
            (2, 12, EventKind::Key, Some("b".into())),
            (2, 12, EventKind::Key, Some("c".into())),
        ]
    );
}

#[gpui::test]
fn r8_a_faded_region_still_draws_but_takes_no_taps_or_keys(cx: &mut TestAppContext) {
    let (view, cx) = host(cx, Fixture::Form);
    tap(cx, 2);
    assert_eq!(events(&view, cx), [(2, 11, EventKind::Click)]);
    view.update(cx, |host, cx| {
        host.state.faded = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(frame(cx, 4), (8., 76., 80., 36.));
    tap(cx, 4);
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(events(&view, cx).is_empty());
}
