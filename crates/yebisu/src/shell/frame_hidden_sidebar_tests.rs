//! Issue #166: with the sidebar as hidden as it gets, the first tab stays
//! clear of iPadOS 26's window controls, in an iPad window, in full screen
//! and on a Mac.
//!
//! The frame has no state without a sidebar column: hiding the sidebar
//! (Command-B, the header button) collapses it to the 72 pt rail at any
//! width (`render_rail` always draws), and a window too narrow for the rail
//! beside a pane switches to the compact frame, whose top bar has no tab row.
//! These tests pin both: from the compact threshold up, a column is painted
//! and the tab row starts past the controls' reach (the rail's 72 pt is wider
//! than their 66 pt), and below it the compact top bar pads its first button
//! by the reach.
use super::overlay_tests::terminal;
use crate::{app::Smoke, layout::RAIL_WIDTH};
use gpui::{
    Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, WindowInsets, px, size,
};

const HEIGHT: f32 = 750.0;
/// Widths that draw the full frame: the compact threshold (the rail beside
/// a pane's 280 pt minimum), one just under the full sidebar's minimum, one
/// just over it, an iPad's portrait width and the owner's external-display
/// window (`frame_top_band_tests.rs`).
const FRAME_WIDTHS: [f32; 5] = [352.0, 580.0, 600.0, 1024.0, 1467.0];
/// Widths that draw the compact frame.
const COMPACT_WIDTHS: [f32; 2] = [320.0, 351.0];
/// The top safe area of the owner's window on an external display (holding
/// the status bar), of full screen, and of a window clear of the screen's
/// top edge (`frame_top_band_tests.rs`).
const WINDOW_TOP: f32 = 32.0;
const FULL_SCREEN_TOP: f32 = 24.0;
const CLEAR_TOP: f32 = 10.0;
/// herdr-uikit's samples (left, right): iPadOS 26.5 window controls, the
/// same in a right-to-left system, a full-screen corner allowance and a
/// Mac's traffic lights (`frame_controls_tests.rs`,
/// `frame_traffic_lights_tests.rs`).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const MAC: (f32, f32) = (103.92, 0.0);

#[derive(Clone, Copy)]
struct Case {
    width: f32,
    top: f32,
    reach: (f32, f32),
    traffic_lights: bool,
}

/// Draws the frame at `window` with the sidebar hidden (`board_hidden`, the
/// saved Command-B choice) or shown.
fn draw(view: &Entity<Smoke>, cx: &mut VisualTestContext, window: &Case, hidden: bool) {
    cx.simulate_resize(size(px(window.width), px(HEIGHT)));
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(WindowInsets {
            safe_area: Edges {
                top: px(window.top),
                ..Edges::default()
            },
            ..WindowInsets::default()
        });
        this.shell.fit.window_controls = Some(Edges {
            left: px(window.reach.0),
            right: px(window.reach.1),
            ..Edges::default()
        });
        this.shell.fit.traffic_lights_over_content = window.traffic_lights;
        this.actions.board_hidden = hidden;
        cx.notify();
    });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

/// The tab row's items that can come first: the space switcher (Herdr 1
/// with the rail) and the first tab.
const FIRST_ITEMS: [&str; 2] = ["shell-space-switcher", "shell-tab-0"];

/// Asserts the frame at `window` keeps a sidebar column with the sidebar
/// hidden or shown, and that the tab row's first items start at or past
/// `leading`.
fn assert_clear(view: &Entity<Smoke>, cx: &mut VisualTestContext, window: &Case, leading: f32) {
    for hidden in [true, false] {
        draw(view, cx, window, hidden);
        let width = window.width;
        let column = cx
            .debug_bounds("rail-item-0")
            .or_else(|| cx.debug_bounds("shell-sidebar-toggle"));
        assert!(
            column.is_some(),
            "a sidebar column is painted at {width} pt (hidden {hidden})"
        );
        let row = bounds(cx, "shell-tab-row");
        assert!(
            row.left() >= px(RAIL_WIDTH),
            "the tab row starts past the rail at {width} pt (hidden {hidden}), at {:?}",
            row.left()
        );
        for selector in FIRST_ITEMS {
            if let Some(item) = cx.debug_bounds(selector) {
                assert!(
                    item.left() >= px(leading),
                    "{selector} starts past the controls' {leading} pt at {width} pt \
                     (hidden {hidden}), at {:?}",
                    item.left()
                );
            }
        }
        assert!(
            cx.debug_bounds("shell-tab-0").is_some(),
            "the first tab is painted at {width} pt (hidden {hidden})"
        );
    }
}

#[gpui::test]
fn an_ipad_window_hides_the_sidebar_to_the_rail_past_the_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for width in FRAME_WIDTHS {
        // The tab row moves up into the band beside the controls (#202),
        // whether or not the band holds the status bar.
        for top in [WINDOW_TOP, CLEAR_TOP] {
            let window = Case {
                width,
                top,
                reach: CONTROLS,
                traffic_lights: false,
            };
            assert_clear(&view, cx, &window, CONTROLS.0);
            // Controls on the right leave the row's start to the rail.
            let right = Case {
                reach: CONTROLS_RIGHT,
                ..window
            };
            assert_clear(&view, cx, &right, 0.0);
        }
    }
}

#[gpui::test]
fn full_screen_and_a_mac_hide_the_sidebar_to_the_rail(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for width in FRAME_WIDTHS {
        let full_screen = Case {
            width,
            top: FULL_SCREEN_TOP,
            reach: FULL_SCREEN,
            traffic_lights: false,
        };
        assert_clear(&view, cx, &full_screen, 0.0);
        let mac = Case {
            width,
            top: 0.0,
            reach: MAC,
            traffic_lights: true,
        };
        assert_clear(&view, cx, &mac, MAC.0);
    }
}

#[gpui::test]
fn a_compact_ipad_window_starts_its_top_bar_past_the_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    for width in COMPACT_WIDTHS {
        let plain = Case {
            width,
            top: WINDOW_TOP,
            reach: (0.0, 0.0),
            traffic_lights: false,
        };
        draw(&view, cx, &plain, true);
        assert!(
            cx.debug_bounds("rail-item-0").is_none() && cx.debug_bounds("shell-tab-row").is_none(),
            "positive control: below the threshold no sidebar column or tab row is painted"
        );
        assert!(
            bounds(cx, "shell-compact-menu").left() < px(CONTROLS.0),
            "positive control: without controls the menu button sits at the edge"
        );
        let more = bounds(cx, "shell-compact-more-actions");
        assert!(
            more.right() > px(width - CONTROLS_RIGHT.1),
            "positive control: without controls ... sits at the right edge, at {:?}",
            more.right()
        );
        for top in [WINDOW_TOP, CLEAR_TOP] {
            let window = Case {
                top,
                reach: CONTROLS,
                ..plain
            };
            draw(&view, cx, &window, true);
            let menu = bounds(cx, "shell-compact-menu");
            assert!(
                menu.left() >= px(CONTROLS.0),
                "the menu button starts past the controls at {width} pt, at {:?}",
                menu.left()
            );
            let right = Case {
                reach: CONTROLS_RIGHT,
                ..window
            };
            draw(&view, cx, &right, true);
            let more = bounds(cx, "shell-compact-more-actions");
            assert!(
                more.right() <= px(width - CONTROLS_RIGHT.1),
                "... ends before the right-to-left controls at {width} pt, at {:?}",
                more.right()
            );
        }
    }
}
