//! The top band of an iPad window (owner report 2026-11-26, Q1 B): with
//! window controls showing, the tab row moves up into the band above it,
//! beside the controls, and its pinned buttons keep clear of the status
//! bar's icons when the band holds the status bar. Full screen (no controls)
//! and a Mac (traffic lights) keep the top safe area exactly as before.
use super::overlay_tests::terminal;
use crate::{app::Smoke, layout::STATUS_ICONS_REACH};
use gpui::{
    Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets, px, size,
};

/// The owner's external-display window at 1.44 px per point: about 1467 by
/// 750 pt, with a 32 pt band above the tab row.
const WIDTH: f32 = 1467.0;
const HEIGHT: f32 = 750.0;
const WINDOW_TOP: f32 = 32.0;
/// Full screen on an iPad: the status bar's 24 pt.
const FULL_SCREEN_TOP: f32 = 24.0;
/// A window clear of the screen's top edge (iPadOS 26.5 Simulator).
const CLEAR_TOP: f32 = 10.0;
/// herdr-uikit's samples (left, right): iPadOS 26.5 window controls, a
/// full-screen corner allowance, and a Mac's traffic lights
/// (`frame_controls_tests.rs`, `frame_traffic_lights_tests.rs`).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const MAC: (f32, f32) = (103.92, 0.0);

fn draw(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    top: f32,
    (left, right): (f32, f32),
    traffic_lights: bool,
    rail: bool,
) {
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            safe_area: Edges {
                top: px(top),
                ..Edges::default()
            },
            ..WindowInsets::default()
        });
        this.shell.fit.window_controls = Some(Edges {
            left: px(left),
            right: px(right),
            ..Edges::default()
        });
        this.shell.fit.traffic_lights_over_content = traffic_lights;
        this.actions.board_hidden = rail;
        cx.notify();
    });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

fn sized(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = terminal(cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    (view, cx)
}

#[gpui::test]
fn a_window_with_controls_puts_the_tab_row_in_the_top_band(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for rail in [true, false] {
        draw(&view, cx, WINDOW_TOP, CONTROLS, false, rail);
        assert_eq!(
            bounds(cx, "shell-tab-row").top(),
            px(0.0),
            "the tab row starts at the window's top edge (rail {rail})"
        );
        let pinned = bounds(cx, "shell-tab-pinned");
        assert!(
            pinned.right() <= px(WIDTH - STATUS_ICONS_REACH),
            "+, split and ... end {:?} before the status icons' {STATUS_ICONS_REACH} pt",
            pinned.right()
        );
    }
}

#[gpui::test]
fn the_controls_stay_beside_the_moved_row(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    // The full sidebar's header and the rail's empty band take the top row's
    // left end, where the controls are; the tab row starts past both.
    draw(&view, cx, WINDOW_TOP, CONTROLS, false, false);
    assert!(bounds(cx, "shell-sidebar-toggle").left() >= px(CONTROLS.0));
    draw(&view, cx, WINDOW_TOP, CONTROLS, false, true);
    assert!(bounds(cx, "shell-tab-row").left() >= px(CONTROLS.0));
}

#[gpui::test]
fn a_window_clear_of_the_status_bar_keeps_its_pinned_buttons(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    draw(&view, cx, CLEAR_TOP, FULL_SCREEN, false, true);
    let plain = bounds(cx, "shell-tab-pinned");
    draw(&view, cx, CLEAR_TOP, CONTROLS, false, true);
    assert_eq!(bounds(cx, "shell-tab-row").top(), px(0.0));
    assert_eq!(
        bounds(cx, "shell-tab-pinned").right(),
        plain.right(),
        "no status bar in the band: the pinned buttons stay at the end"
    );
}

#[gpui::test]
fn controls_on_the_right_keep_the_header_clear_of_the_status_icons(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    draw(&view, cx, WINDOW_TOP, CONTROLS_RIGHT, false, false);
    assert_eq!(bounds(cx, "shell-tab-row").top(), px(0.0));
    assert!(bounds(cx, "shell-sidebar-toggle").left() >= px(STATUS_ICONS_REACH));
}

#[gpui::test]
fn full_screen_keeps_the_status_bar_band(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for rail in [true, false] {
        draw(&view, cx, FULL_SCREEN_TOP, FULL_SCREEN, false, rail);
        assert_eq!(
            bounds(cx, "shell-tab-row").top(),
            px(FULL_SCREEN_TOP),
            "no controls: the tab row stays below the status bar (rail {rail})"
        );
        let plain = bounds(cx, "shell-tab-pinned");
        draw(&view, cx, FULL_SCREEN_TOP, (0.0, 0.0), false, rail);
        assert_eq!(bounds(cx, "shell-tab-pinned"), plain);
    }
}

#[gpui::test]
fn a_mac_keeps_its_top_row(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    // herdr-uikit reports 0 on a Mac with the content under the title bar;
    // any other value must reach the layout unchanged too.
    for top in [0.0, WINDOW_TOP] {
        draw(&view, cx, top, (0.0, 0.0), false, true);
        let plain = bounds(cx, "shell-tab-pinned");
        draw(&view, cx, top, MAC, true, true);
        assert_eq!(bounds(cx, "shell-tab-row").top(), px(top));
        assert_eq!(
            bounds(cx, "shell-tab-pinned"),
            plain,
            "no status-icon clearance on a Mac"
        );
    }
}
