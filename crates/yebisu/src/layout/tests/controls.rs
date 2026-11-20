//! iPadOS 26's window controls (`controls.rs`): the window keeps clear of
//! them on whichever side they show, nothing else moves, and neither a corner
//! allowance alone nor a window not measured yet changes anything.
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
    left: 9.5,
    right: 66.0,
};

/// A window with a 10 pt top safe area (the narrow Simulator window) at
/// `width`: the rail at 375, the full sidebar at 1032.
fn at(width: f32, reach: Option<ControlsReach>) -> Input {
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
    let plain = layout(at(width, None));
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
        let out = layout(at(width, Some(CONTROLS)));
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
    assert!(layout(at(375.0, Some(CONTROLS))).sidebar_collapsed);
    assert!(!layout(at(1032.0, Some(CONTROLS))).sidebar_collapsed);
}

#[test]
fn window_controls_on_the_right_clear_the_tab_row_end() {
    for width in [375.0, 1032.0] {
        let out = layout(at(width, Some(CONTROLS_RIGHT)));
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
    let reach = |left, right| Some(ControlsReach { left, right });
    let samples = [
        Some(FULL_SCREEN),
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
            assert_eq!(out, layout(at(width, None)), "{sample:?} {width}");
        }
    }
}

#[test]
fn a_window_not_measured_yet_keeps_the_design_sizes() {
    for width in [375.0, 1032.0] {
        let out = layout(at(width, None));
        assert_eq!(out.controls, ControlsClearance::default(), "{width}");
    }
}
