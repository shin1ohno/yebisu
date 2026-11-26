//! iPadOS 26's window controls (`controls.rs`): the window keeps clear of
//! them on whichever side they show, nothing else moves, and neither a corner
//! allowance alone nor all zero (no controls) changes anything.
use super::super::{
    ControlsClearance, ControlsReach, Input, Insets, Layout, SIDEBAR_HEADER_HEIGHT,
    SIDEBAR_HEADER_PADDING,
};
use super::{layout, window};

/// UIKit's reach (left, right) on the iPadOS 26.5 Simulator, iPad Pro
/// 13-inch (M5), from the app's `window_controls` record: a window with the
/// controls showing, and a full-screen window.
const CONTROLS: ControlsReach = ControlsReach {
    left: 66.0,
    right: 9.5,
};
const FULL_SCREEN: ControlsReach = ControlsReach {
    left: 9.5,
    right: 9.0,
};
/// The same window in a right-to-left system: the controls on the right.
const CONTROLS_RIGHT: ControlsReach = ControlsReach {
    left: 9.0,
    right: 66.0,
};
/// All zero: no window controls.
const NONE: ControlsReach = ControlsReach {
    left: 0.0,
    right: 0.0,
};

/// A window with a 10 pt top safe area (the narrow Simulator window) at
/// `width`: the rail at 375, the full sidebar at 1032.
fn at(width: f32, reach: ControlsReach) -> Input {
    let bare = window(width, 834.0);
    Input {
        insets: Insets {
            safe_top: 10.0,
            window_controls: reach,
            ..bare.insets
        },
        ..bare
    }
}

/// Every rectangle of `out` equals the one without controls: only the
/// clearance inside the sidebar and the tab row may differ.
fn only_clearance_differs(out: Layout, width: f32) {
    let plain = layout(at(width, NONE));
    assert_eq!(
        Layout {
            controls: plain.controls,
            ..out
        },
        plain,
        "{width}"
    );
}

#[test]
fn window_controls_clear_sidebar_header_and_rail() {
    for width in [375.0, 1032.0] {
        let out = layout(at(width, CONTROLS));
        assert_eq!(
            out.controls,
            ControlsClearance {
                header_leading: CONTROLS.left,
                rail_top: SIDEBAR_HEADER_HEIGHT,
                tab_row_trailing: 0.0,
            },
            "{width}"
        );
        // The tab row starts right of the sidebar column, past the controls.
        assert!(out.tab_row.x >= out.sidebar.x + CONTROLS.left, "{width}");
        only_clearance_differs(out, width);
    }
    assert!(layout(at(375.0, CONTROLS)).sidebar_collapsed);
    assert!(!layout(at(1032.0, CONTROLS)).sidebar_collapsed);
}

#[test]
fn window_controls_on_the_right_clear_the_tab_row_end() {
    for width in [375.0, 1032.0] {
        let out = layout(at(width, CONTROLS_RIGHT));
        assert_eq!(
            out.controls,
            ControlsClearance {
                header_leading: SIDEBAR_HEADER_PADDING,
                rail_top: 0.0,
                tab_row_trailing: CONTROLS_RIGHT.right,
            },
            "{width}"
        );
        // The tab row runs to the window's right edge, under the controls.
        assert_eq!(out.tab_row.right(), width, "{width}");
        only_clearance_differs(out, width);
    }
}

#[test]
fn corner_allowance_alone_moves_nothing() {
    assert_eq!(
        ControlsClearance::default(),
        ControlsClearance {
            header_leading: SIDEBAR_HEADER_PADDING,
            rail_top: 0.0,
            tab_row_trailing: 0.0,
        }
    );
    let reach = |left, right| ControlsReach { left, right };
    let samples = [
        FULL_SCREEN,
        // herdr-uikit's full-width sample.
        reach(5.5, 5.5),
        // A larger allowance than any measured, the same on both corners.
        reach(13.0, 13.0),
        reach(40.0, 40.0),
        // Corners that differ by no more than the threshold.
        reach(21.5, 9.5),
        reach(9.5, 21.5),
        reach(-3.0, f32::NAN),
        reach(f32::NAN, f32::NAN),
    ];
    for sample in samples {
        for width in [375.0, 1032.0] {
            let out = layout(at(width, sample));
            assert_eq!(
                out.controls,
                ControlsClearance::default(),
                "{sample:?} {width}"
            );
            assert_eq!(out, layout(at(width, NONE)), "{sample:?} {width}");
        }
    }
}

/// All zero is a window without controls (owner decision 2026-11-20):
/// no live scene, or not sampled yet. It keeps the design
/// sizes, like `Insets::default()`.
#[test]
fn all_zero_is_no_window_controls() {
    assert_eq!(Insets::default().window_controls, NONE);
    for width in [375.0, 1032.0] {
        let out = layout(at(width, NONE));
        assert_eq!(out.controls, ControlsClearance::default(), "{width}");
    }
}

/// The 12 pt threshold against the measured values (`controls.rs`): the
/// smallest measured gap with controls (56.5 pt) clears, the largest without
/// (0.5 pt) does not, the threshold itself is exclusive, and an asymmetric
/// pair no UIKit configuration has reported (left 52 / right 64, the
/// review's example) gets no clearance.
#[test]
fn the_threshold_splits_measured_gaps() {
    let reach = |left, right| ControlsReach { left, right };
    let left_side = |reach: f32| ControlsClearance {
        header_leading: reach,
        rail_top: SIDEBAR_HEADER_HEIGHT,
        tab_row_trailing: 0.0,
    };
    let right_side = |reach: f32| ControlsClearance {
        tab_row_trailing: reach,
        ..ControlsClearance::default()
    };
    let none = ControlsClearance::default();
    let cases = [
        (CONTROLS, left_side(66.0)),
        (CONTROLS_RIGHT, right_side(66.0)),
        (FULL_SCREEN, none),
        (reach(21.75, 9.5), left_side(21.75)),
        (reach(9.5, 21.5), none),
        (reach(52.0, 64.0), none),
        (reach(64.0, 52.0), none),
    ];
    for (sample, expected) in cases {
        assert_eq!(layout(at(1032.0, sample)).controls, expected, "{sample:?}");
    }
}

/// herdr-uikit's reach on a Mac with its traffic lights over the content
/// (macOS 26.7.3): the Mac's 80 pt at its 0.77 scale on the left.
const TRAFFIC_LIGHTS: ControlsReach = ControlsReach {
    left: 103.92,
    right: 0.0,
};

#[test]
fn the_rail_pads_its_tab_row_past_the_traffic_lights() {
    let clearance = TRAFFIC_LIGHTS.clearance();
    // The rail (72 pt) and its rule: the tab row starts at 73.
    let leading =
        clearance.tab_row_leading(true, 0.0, super::super::RAIL_WIDTH + super::super::RULE);
    assert!((leading - (103.92 - 73.0)).abs() < 1e-3, "{leading}");
}

#[test]
fn no_sidebar_pads_its_tab_row_by_the_whole_reach() {
    assert_eq!(
        TRAFFIC_LIGHTS.clearance().tab_row_leading(true, 0.0, 0.0),
        103.92
    );
}

#[test]
fn the_full_sidebar_tab_row_needs_no_padding() {
    let start = super::super::SIDEBAR_WIDTH + super::super::RULE;
    assert_eq!(
        TRAFFIC_LIGHTS.clearance().tab_row_leading(true, 0.0, start),
        0.0
    );
}

#[test]
fn an_ipad_never_pads_its_tab_row() {
    // The flag is false on an iPad, whatever the controls reach and wherever
    // the tab row starts, including with no sidebar at all.
    for reach in [CONTROLS, FULL_SCREEN, CONTROLS_RIGHT, NONE, TRAFFIC_LIGHTS] {
        for start in [0.0, 73.0, 301.0] {
            assert_eq!(reach.clearance().tab_row_leading(false, 0.0, start), 0.0);
        }
    }
}

#[test]
fn traffic_lights_on_the_right_leave_the_tab_row_start_alone() {
    // A right-to-left Mac: the trailing clearance covers them.
    let right = ControlsReach {
        left: 0.0,
        right: 103.92,
    };
    assert_eq!(right.clearance().tab_row_leading(true, 0.0, 0.0), 0.0);
}

/// The owner's iPad window on an external display (2026-11-26): a 32 pt top
/// band holding the status bar.
const WINDOW_TOP: f32 = 32.0;

#[test]
fn a_window_with_controls_moves_its_top_row_into_the_band() {
    let band = super::super::top_band(WINDOW_TOP, true);
    assert_eq!(band.safe_top, 0.0);
    assert_eq!(band.status_icons, super::super::STATUS_ICONS_REACH);
    // Clear of the screen's top edge (10 pt on the iPadOS 26.5 Simulator)
    // there is no status bar to keep clear of.
    let clear = super::super::top_band(10.0, true);
    assert_eq!((clear.safe_top, clear.status_icons), (0.0, 0.0));
}

#[test]
fn full_screen_and_a_mac_keep_the_reported_top() {
    for top in [0.0, 24.0, WINDOW_TOP] {
        let band = super::super::top_band(top, false);
        assert_eq!((band.safe_top, band.status_icons), (top, 0.0));
    }
    let nan = super::super::top_band(f32::NAN, false);
    assert_eq!((nan.safe_top, nan.status_icons), (0.0, 0.0));
}

#[test]
fn status_icons_sit_opposite_the_controls() {
    let icons = super::super::STATUS_ICONS_REACH;
    let left = layout(Input {
        insets: Insets {
            window_controls: CONTROLS,
            status_icons: icons,
            ..Insets::default()
        },
        ..at(1467.0, CONTROLS)
    });
    assert_eq!(
        left.controls,
        ControlsClearance {
            header_leading: CONTROLS.left,
            rail_top: SIDEBAR_HEADER_HEIGHT,
            tab_row_trailing: icons,
        }
    );
    let right = layout(Input {
        insets: Insets {
            window_controls: CONTROLS_RIGHT,
            status_icons: icons,
            ..Insets::default()
        },
        ..at(1467.0, CONTROLS_RIGHT)
    });
    assert_eq!(
        right.controls,
        ControlsClearance {
            header_leading: icons,
            rail_top: SIDEBAR_HEADER_HEIGHT,
            tab_row_trailing: CONTROLS_RIGHT.right,
        }
    );
    // Without controls the icons never reach the layout (`top_band`), and
    // the controls alone keep today's clearance.
    assert_eq!(
        CONTROLS_RIGHT.clearance().header_leading,
        SIDEBAR_HEADER_PADDING
    );
    assert_eq!(CONTROLS.clearance().tab_row_trailing, 0.0);
    assert!(!FULL_SCREEN.clearance().shows_controls());
    assert!(!NONE.clearance().shows_controls());
    assert!(CONTROLS.clearance().shows_controls());
    assert!(CONTROLS_RIGHT.clearance().shows_controls());
}
