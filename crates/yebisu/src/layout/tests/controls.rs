//! iPadOS 26's window controls (`controls.rs`): the sidebar keeps clear of
//! them, nothing else moves, and a corner allowance alone changes nothing.
use super::super::{
    ControlsClearance, Input, Insets, Layout, SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING,
};
use super::{layout, window};

/// UIKit's reach on iPadOS 26.5: the left edge of a window with the controls
/// showing, the corner allowance alone (right edge of that window, and the
/// left edge of a full-screen window on the iPad Pro 13-inch (M5)
/// Simulator), and the smallest allowance herdr-uikit measured.
const CONTROLS: f32 = 66.0;
const WINDOW_ALLOWANCE: f32 = 9.5;
const FULL_SCREEN_ALLOWANCE: f32 = 5.5;

/// A window with a 10 pt top safe area (the narrow Simulator window) at
/// `width`: the rail at 375, the full sidebar at 1032.
fn at(width: f32, reach: f32) -> Input {
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

#[test]
fn window_controls_clear_sidebar_header_and_rail() {
    for width in [375.0, 1032.0] {
        let out = layout(at(width, CONTROLS));
        assert_eq!(
            out.controls,
            ControlsClearance {
                header_leading: CONTROLS,
                rail_top: SIDEBAR_HEADER_HEIGHT,
            },
            "{width}"
        );
        // The tab row starts right of the sidebar column, past the controls.
        assert!(out.tab_row.x >= out.sidebar.x + CONTROLS, "{width}");
        // Only the sidebar's insides move: every rectangle stays put.
        let plain = layout(at(width, 0.0));
        assert_eq!(
            Layout {
                controls: plain.controls,
                ..out
            },
            plain,
            "{width}"
        );
    }
    assert!(layout(at(375.0, CONTROLS)).sidebar_collapsed);
    assert!(!layout(at(1032.0, CONTROLS)).sidebar_collapsed);
}

#[test]
fn corner_allowance_alone_moves_nothing() {
    assert_eq!(
        ControlsClearance::default(),
        ControlsClearance {
            header_leading: SIDEBAR_HEADER_PADDING,
            rail_top: 0.0,
        }
    );
    let reaches = [
        0.0,
        FULL_SCREEN_ALLOWANCE,
        WINDOW_ALLOWANCE,
        SIDEBAR_HEADER_PADDING,
        -3.0,
        f32::NAN,
    ];
    for reach in reaches {
        for width in [375.0, 1032.0] {
            let out = layout(at(width, reach));
            assert_eq!(
                out.controls,
                ControlsClearance::default(),
                "{reach} {width}"
            );
            assert_eq!(out, layout(at(width, 0.0)), "{reach} {width}");
        }
    }
}
