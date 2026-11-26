//! The bottom band of an iPad window (owner report 2026-11-26, Q2 B): with
//! window controls showing, the status bar moves down to the window's bottom
//! edge instead of stopping above the bottom safe area. Full screen (no
//! controls) and a Mac (traffic lights) keep the bottom safe area as before.
use super::overlay_tests::terminal;
use crate::app::Smoke;
use gpui::{
    Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets, px, size,
};

/// The owner's external-display window at 1.44 px per point: about 1467 by
/// 750 pt, with a 25 pt band below the status bar.
const WIDTH: f32 = 1467.0;
const HEIGHT: f32 = 750.0;
const WINDOW_BOTTOM: f32 = 25.0;
/// Full screen on an iPad: the home indicator's 20 pt.
const FULL_SCREEN_BOTTOM: f32 = 20.0;
/// herdr-uikit's samples (left, right): iPadOS 26.5 window controls, a
/// full-screen corner allowance, right-to-left controls and a Mac's traffic
/// lights (`frame_controls_tests.rs`, `frame_traffic_lights_tests.rs`).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const MAC: (f32, f32) = (103.92, 0.0);

fn draw(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    bottom: f32,
    (left, right): (f32, f32),
    traffic_lights: bool,
    rail: bool,
) {
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            safe_area: Edges {
                bottom: px(bottom),
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

/// The status line with no bottom safe area at all: where it sits when the
/// status bar reaches the window's bottom edge.
fn at_the_edge(view: &Entity<Smoke>, cx: &mut VisualTestContext, rail: bool) -> Bounds<Pixels> {
    draw(view, cx, 0.0, (0.0, 0.0), false, rail);
    bounds(cx, "shell-status-line")
}

#[gpui::test]
fn a_window_with_controls_puts_the_status_bar_at_its_bottom_edge(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for rail in [true, false] {
        let edge = at_the_edge(&view, cx, rail);
        for controls in [CONTROLS, CONTROLS_RIGHT] {
            draw(&view, cx, WINDOW_BOTTOM, controls, false, rail);
            assert_eq!(
                bounds(cx, "shell-status-line"),
                edge,
                "the status bar fills the {WINDOW_BOTTOM} pt band (rail {rail}, {controls:?})"
            );
        }
    }
}

#[gpui::test]
fn full_screen_keeps_the_home_indicator_band(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for rail in [true, false] {
        let edge = at_the_edge(&view, cx, rail);
        draw(&view, cx, FULL_SCREEN_BOTTOM, FULL_SCREEN, false, rail);
        assert_eq!(
            bounds(cx, "shell-status-line").bottom(),
            edge.bottom() - px(FULL_SCREEN_BOTTOM),
            "no controls: the status bar stays above the home indicator (rail {rail})"
        );
    }
}

#[gpui::test]
fn a_mac_keeps_its_bottom_safe_area(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    let edge = at_the_edge(&view, cx, true);
    draw(&view, cx, WINDOW_BOTTOM, MAC, true, true);
    assert_eq!(
        bounds(cx, "shell-status-line").bottom(),
        edge.bottom() - px(WINDOW_BOTTOM),
        "a Mac's traffic lights are not iPad window controls"
    );
}
