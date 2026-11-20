//! iPadOS 26's window controls over a top corner of the window (L0
//! `layout/controls.rs`): the painted sidebar keeps its header content and
//! the rail's first item clear of them, the tab row keeps its pinned buttons
//! clear of controls on the right, a full-screen window's corner allowance
//! moves nothing, all zero is no controls, and the reach is read on every
//! frame through `platform_window_controls` (fit.rs).
//!
//! The UIKit -> GPUI -> draw path cannot run here: herdr-uikit is built for
//! iOS only (`window_control_insets` behind `cfg(target_os = "ios")`), and
//! GPUI 0.3.12's `TestWindow::simulate_insets_change`, which would run the
//! window's insets-changed handler, is reachable only through the
//! crate-private `TestAppContext::test_window`. The closest seam is tested:
//! the sample changes with no notify and no resize, and the window gets what
//! that handler does (`window.refresh()`, GPUI `src/window.rs:2112-2121`).
//! The Simulator check in PR 87 covers the native path.
use super::{fit::controls_reach, overlay_tests::terminal};
use crate::{
    app::Smoke,
    layout::{ControlsClearance, ControlsReach, SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING},
    observation::Observation,
    shell::{
        rail::RAIL_HEADER_HEIGHT,
        tab_row::{PINNED_ACTION_WIDTH, TabHit, tab_hit},
    },
};
use gpui::{
    Bounds, Edges, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, point, px, size,
};
use std::{
    sync::mpsc::sync_channel,
    time::{Duration, Instant},
};

/// herdr-uikit's samples (left, right) on the iPadOS 26.5 Simulator (iPad
/// Pro 13-inch (M5), the app's `window_controls` record): a window with the
/// controls showing, a full-screen window, the same window in a
/// right-to-left system, and all zero (no controls: iPadOS 25, or a window
/// not sampled yet).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const NONE: (f32, f32) = (0.0, 0.0);

fn sample((left, right): (f32, f32)) -> Edges<Pixels> {
    Edges {
        left: px(left),
        right: px(right),
        ..Edges::default()
    }
}

/// Gives the window the SDK sample `reach` without notifying the view: only
/// a platform refresh (an insets change, a resize) draws it.
fn set_sample(view: &Entity<Smoke>, cx: &mut VisualTestContext, reach: (f32, f32)) {
    view.update(cx, |this, _| {
        this.shell.fit.window_controls = Some(sample(reach))
    });
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

/// Draws the frame with the SDK sample `reach`, the sidebar as the rail or
/// the full column, and returns where `selector` was painted.
fn painted(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    reach: (f32, f32),
    rail: bool,
    selector: &'static str,
) -> Bounds<Pixels> {
    set_sample(view, cx, reach);
    view.update(cx, |this, cx| {
        this.actions.board_hidden = rail;
        cx.notify();
    });
    cx.run_until_parked();
    bounds(cx, selector)
}

/// The SDK sample's left and right edges are the reach; all zero is no
/// window controls (owner decision 2026-11-20).
#[test]
fn an_all_zero_sample_is_no_window_controls() {
    assert_eq!(controls_reach(sample(NONE)), ControlsReach::default());
    assert_eq!(
        controls_reach(sample(CONTROLS)),
        ControlsReach {
            left: 66.0,
            right: 9.5
        }
    );
    let top_only = Edges {
        top: px(1.0),
        ..Edges::default()
    };
    assert_eq!(
        controls_reach(top_only),
        ControlsReach::default(),
        "the reach is horizontal only"
    );
}

#[gpui::test]
fn rail_first_item_starts_under_the_window_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let none = painted(&view, cx, NONE, true, "rail-item-0");
    assert_eq!(
        none.origin.y,
        px(RAIL_HEADER_HEIGHT),
        "positive control: without controls the list starts under the 8 pt header"
    );
    let full_screen = painted(&view, cx, FULL_SCREEN, true, "rail-item-0");
    assert_eq!(
        full_screen.origin, none.origin,
        "a full-screen window's corner allowance must not move the rail"
    );
    let controls = painted(&view, cx, CONTROLS, true, "rail-item-0");
    assert_eq!(
        controls.origin.y,
        px(SIDEBAR_HEADER_HEIGHT),
        "the first item must start under the header band the controls sit in, \
         where the full sidebar's list starts"
    );
}

#[gpui::test]
fn full_sidebar_header_starts_past_the_window_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let field = "sidebar-filter-unavailable";
    let none = painted(&view, cx, NONE, false, field);
    assert_eq!(
        none.origin.x,
        px(SIDEBAR_HEADER_PADDING),
        "positive control: without controls the header keeps its 12 pt padding"
    );
    let full_screen = painted(&view, cx, FULL_SCREEN, false, field);
    assert_eq!(
        full_screen.origin, none.origin,
        "a full-screen window's corner allowance must not move the header"
    );
    let controls = painted(&view, cx, CONTROLS, false, field);
    assert_eq!(
        controls.origin.x,
        px(CONTROLS.0),
        "the header's content must start past the controls"
    );
    assert_eq!(
        controls.origin.y, none.origin.y,
        "only the header's content moves; the header keeps its place and height"
    );
}

#[gpui::test]
fn tab_row_pinned_buttons_clear_window_controls_on_the_right(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let width = cx.update(|window, _| window.viewport_size().width);
    let pinned = "shell-tab-pinned";
    let none = painted(&view, cx, NONE, false, pinned);
    assert_eq!(
        none.right(),
        width,
        "positive control: the pinned buttons end at the window's right edge"
    );
    let left = painted(&view, cx, CONTROLS, false, pinned);
    assert_eq!(left, none, "controls on the left leave the tab row alone");
    for rail in [false, true] {
        let right = painted(&view, cx, CONTROLS_RIGHT, rail, pinned);
        assert_eq!(
            right.right(),
            width - px(CONTROLS_RIGHT.1),
            "rail {rail}: the pinned buttons must end before the controls"
        );
        assert_eq!(
            right.size, none.size,
            "rail {rail}: the buttons keep their size"
        );
    }
    let field = painted(
        &view,
        cx,
        CONTROLS_RIGHT,
        false,
        "sidebar-filter-unavailable",
    );
    assert_eq!(
        field.origin.x,
        px(SIDEBAR_HEADER_PADDING),
        "controls on the right leave the sidebar header alone"
    );
}

/// The tab row's hit model (`tab_hit`) agrees with the drawn row under a
/// right clearance of 66 pt (review of PR 87 at 16ef79e): a tap on the
/// moved "..." opens the overflow menu, a tap where it used to be does not,
/// and the model says the same at both places.
#[gpui::test]
fn the_moved_overflow_button_takes_the_tap(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let pinned = painted(&view, cx, CONTROLS_RIGHT, false, "shell-tab-pinned");
    let row = bounds(cx, "shell-tab-row");
    assert_eq!(pinned.right(), row.right() - px(CONTROLS_RIGHT.1));
    let y = pinned.center().y;
    let moved = pinned.right() - px(PINNED_ACTION_WIDTH / 2.0);
    let old = row.right() - px(PINNED_ACTION_WIDTH / 2.0);
    let model = |x: Pixels| {
        tab_hit(
            f32::from(x - row.left()),
            &[],
            f32::from(row.size.width),
            CONTROLS_RIGHT.1,
        )
    };
    assert_eq!(model(moved), Some(TabHit::Overflow));
    assert_eq!(model(old), None);
    let open = |view: &Entity<Smoke>, cx: &mut VisualTestContext| {
        view.read_with(cx, |this, _| this.shell.frame.overflow_open)
    };
    assert!(!open(&view, cx));
    cx.simulate_click(point(old, y), Modifiers::none());
    cx.run_until_parked();
    assert!(
        !open(&view, cx),
        "a tap under the window controls must not open the menu"
    );
    cx.simulate_click(point(moved, y), Modifiers::none());
    cx.run_until_parked();
    assert!(open(&view, cx), "a tap on the moved ... must open the menu");
}

/// What GPUI's insets-changed handler does when herdr-uikit reports a new
/// sample (herdr-uikit `src/window/layout.rs:166` calls the insets callback;
/// GPUI 0.3.12 `src/window.rs:2112-2121` answers with `window.refresh()`).
fn platform_insets_changed(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Start with an all-zero sample and no clearance. The first sample that
/// contains controls arrives with no resize or notify. The platform
/// refresh alone lays the frame out with the new reach.
#[gpui::test]
fn first_measurement_reaches_the_next_frame_without_a_notify(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let before = painted(&view, cx, NONE, true, "rail-item-0");
    assert_eq!(before.origin.y, px(RAIL_HEADER_HEIGHT));
    let recorded = view.read_with(cx, |this, _| this.shell.fit.recorded_controls);
    assert_eq!(
        recorded,
        Some((ControlsReach::default(), ControlsClearance::default()))
    );

    set_sample(&view, cx, CONTROLS);
    cx.run_until_parked();
    assert_eq!(
        bounds(cx, "rail-item-0"),
        before,
        "negative control: a sample alone, with no platform refresh, draws nothing"
    );
    platform_insets_changed(cx);
    let after = bounds(cx, "rail-item-0");
    assert_eq!(after.origin.y, px(SIDEBAR_HEADER_HEIGHT));
    let recorded = view.read_with(cx, |this, _| this.shell.fit.recorded_controls);
    assert_eq!(
        recorded,
        Some((
            ControlsReach {
                left: CONTROLS.0,
                right: CONTROLS.1,
            },
            ControlsClearance {
                header_leading: CONTROLS.0,
                rail_top: SIDEBAR_HEADER_HEIGHT,
                tab_row_trailing: 0.0,
            },
        )),
        "the observation log must get the measured reach and the clearance"
    );
}

/// At a fixed window size, the platform refresh alone moves the tab row's
/// buttons when right-side controls appear and restores them at all zero.
#[gpui::test]
fn a_native_only_change_redraws_at_the_same_size(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    let pinned = "shell-tab-pinned";
    let none = painted(&view, cx, NONE, false, pinned);
    assert_eq!(none.right(), viewport.width);

    set_sample(&view, cx, CONTROLS_RIGHT);
    platform_insets_changed(cx);
    assert_eq!(
        bounds(cx, pinned).right(),
        viewport.width - px(CONTROLS_RIGHT.1),
        "controls appearing on the right move the pinned buttons"
    );
    set_sample(&view, cx, NONE);
    platform_insets_changed(cx);
    assert_eq!(
        bounds(cx, pinned),
        none,
        "all zero again: the pinned buttons go back to the edge"
    );
    assert_eq!(
        cx.update(|window, _| window.viewport_size()),
        viewport,
        "the window size never changed"
    );
}

/// A resize redraws with the reach the platform reports at that moment, read
/// through `platform_window_controls`, not a value kept from an earlier frame.
#[gpui::test]
fn a_resize_lays_out_with_the_current_reach(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let wide = painted(&view, cx, NONE, false, "sidebar-filter-unavailable");
    assert_eq!(wide.origin.x, px(SIDEBAR_HEADER_PADDING));
    set_sample(&view, cx, CONTROLS);
    // Narrower than the full sidebar and one pane need: the rail.
    cx.simulate_resize(size(px(480.), px(900.)));
    cx.run_until_parked();
    let item = bounds(cx, "rail-item-0");
    assert_eq!(item.origin.y, px(SIDEBAR_HEADER_HEIGHT));
}

/// The record is offered while drawing; a full observation queue drops it
/// at once (counted and reported) instead of holding the UI thread for the
/// queue's wait, and the drop line is printed by the reporter thread, not
/// while drawing (`observation/drop_tests.rs`).
#[gpui::test]
fn a_full_observation_queue_does_not_block_drawing(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let (send, records) = sync_channel(2);
    let reach = ControlsReach {
        left: CONTROLS.0,
        right: CONTROLS.1,
    };
    let waited = view.update(cx, |this, _| {
        this.observation = Observation::injected_with_wait(send, Duration::from_secs(2));
        for _ in 0..2 {
            assert!(
                this.observation
                    .try_record(serde_json::json!({"kind": "fill"}))
            );
        }
        let started = Instant::now();
        this.record_window_controls(reach, ControlsClearance::default());
        started.elapsed()
    });
    assert!(
        waited < Duration::from_millis(500),
        "recording waited {waited:?} for a full queue"
    );
    let next = || {
        records
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_else(|e| panic!("{e}"))
    };
    assert_eq!(next()["kind"], "fill");
    assert_eq!(next()["kind"], "fill");
    view.update(cx, |this, _| {
        this.record_window_controls(ControlsReach::default(), ControlsClearance::default())
    });
    let notice = next();
    assert_eq!(notice["kind"], "observation-dropped", "{notice}");
    assert_eq!(notice["count"], 1);
    let record = next();
    assert_eq!(record["kind"], "window_controls");
    assert_eq!(record["left"], 0.0);
    assert_eq!(record["right"], 0.0);
}
