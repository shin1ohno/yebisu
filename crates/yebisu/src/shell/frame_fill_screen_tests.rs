//! Issue #221: a window that fills its screen (full screen, or an iPadOS 26
//! window the user maximized) is not an iPad window for the top and bottom
//! bands, even while UIKit reports its window controls. herdr-uikit hides the
//! OS status bar there (`window_fills_screen`), and the frame keeps to the
//! safe area UIKit reports: below the status bar if it still shows, at the
//! top edge once it is gone. On the owner's TestFlight build 256 the tab row
//! sat in the band under the status bar instead, its time and date over the
//! first tabs. Smaller windows (#202, #209) and a Mac keep their look.
use super::overlay_tests::terminal;
use gpui::{
    Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets, px, size,
};

use crate::app::Smoke;

/// An iPad Pro 13-inch (M5) in landscape, in points: a window filling it.
const WIDTH: f32 = 1376.0;
const HEIGHT: f32 = 1032.0;
/// Full screen's top safe area with the status bar, and its home-indicator
/// band at the bottom (iPadOS 26.5 Simulator, `frame_top_band_tests.rs`).
const STATUS_BAR_TOP: f32 = 24.0;
const HOME_BOTTOM: f32 = 20.0;
/// herdr-uikit's samples (left, right): iPadOS 26.5 window controls, the
/// same right to left, and a Mac's traffic lights (`frame_controls_tests.rs`).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const MAC: (f32, f32) = (103.92, 0.0);

#[derive(Clone, Copy, Debug)]
struct Case {
    top: f32,
    reach: (f32, f32),
    fills_screen: bool,
    traffic_lights: bool,
    rail: bool,
}

fn draw(view: &Entity<Smoke>, cx: &mut VisualTestContext, case: Case) {
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            safe_area: Edges {
                top: px(case.top),
                bottom: px(HOME_BOTTOM),
                ..Edges::default()
            },
            ..WindowInsets::default()
        });
        this.shell.fit.window_controls = Some(Edges {
            left: px(case.reach.0),
            right: px(case.reach.1),
            ..Edges::default()
        });
        this.shell.fit.fills_screen = case.fills_screen;
        this.shell.fit.traffic_lights_over_content = case.traffic_lights;
        this.actions.board_hidden = case.rail;
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

/// The owner's report: controls reported, the status bar still shown. The
/// tab row stays below it, as before #202, with no room kept for the status
/// icons, and the status bar keeps the home-indicator band below it (#203
/// moved it down only in a smaller window).
#[gpui::test]
fn a_window_filling_the_screen_keeps_its_row_below_the_status_bar(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for reach in [CONTROLS, CONTROLS_RIGHT] {
        for rail in [true, false] {
            let filling = Case {
                top: STATUS_BAR_TOP,
                reach,
                fills_screen: true,
                traffic_lights: false,
                rail,
            };
            draw(&view, cx, filling);
            let at = format!("{reach:?}, rail {rail}");
            assert_eq!(
                bounds(cx, "shell-tab-row").top(),
                px(STATUS_BAR_TOP),
                "the tab row stays below the status bar ({at})"
            );
            let line = bounds(cx, "shell-status-line");
            if reach == CONTROLS {
                assert_eq!(
                    bounds(cx, "shell-tab-pinned").right(),
                    px(WIDTH),
                    "no room for the status icons beside the row ({at})"
                );
            }
            // Positive control: the same window not filling its screen is
            // an iPad window, whose row moves up into the band (#202).
            draw(
                &view,
                cx,
                Case {
                    fills_screen: false,
                    ..filling
                },
            );
            assert_eq!(
                bounds(cx, "shell-tab-row").top(),
                px(0.0),
                "a smaller window moves the row into the band ({at})"
            );
            // ... and its status bar down to the bottom edge (#203); the one
            // filling the screen keeps the home-indicator band.
            assert_eq!(
                line.bottom(),
                bounds(cx, "shell-status-line").bottom() - px(HOME_BOTTOM),
                "the status bar keeps the home-indicator band ({at})"
            );
        }
    }
}

/// Once herdr-uikit has hidden the status bar, UIKit reports a top safe area
/// without it, and the row starts there.
#[gpui::test]
fn a_hidden_status_bar_lets_the_row_reach_the_top(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for top in [0.0, 10.0] {
        draw(
            &view,
            cx,
            Case {
                top,
                reach: CONTROLS,
                fills_screen: true,
                traffic_lights: false,
                rail: true,
            },
        );
        assert_eq!(bounds(cx, "shell-tab-row").top(), px(top));
        assert_eq!(bounds(cx, "shell-tab-pinned").right(), px(WIDTH));
    }
}

/// Negative control: a Mac (traffic lights) is never an iPad window, so
/// filling its screen changes nothing.
#[gpui::test]
fn a_mac_filling_its_screen_keeps_its_look(cx: &mut TestAppContext) {
    let (view, cx) = sized(cx);
    for top in [0.0, STATUS_BAR_TOP] {
        let mac = Case {
            top,
            reach: MAC,
            fills_screen: false,
            traffic_lights: true,
            rail: true,
        };
        draw(&view, cx, mac);
        let selectors = ["shell-tab-row", "shell-tab-pinned", "shell-status-line"];
        let plain: Vec<_> = selectors.iter().map(|s| bounds(cx, s)).collect();
        draw(
            &view,
            cx,
            Case {
                fills_screen: true,
                ..mac
            },
        );
        for (selector, plain) in selectors.iter().zip(plain) {
            assert_eq!(bounds(cx, selector), plain, "{selector} (top {top})");
        }
    }
}
