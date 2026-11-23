//! D1: the sheet keeps the canvas position when it fits, otherwise sits
//! 16 pt from the top; below 760 pt of width the sections stack.
use crate::{
    layout::ControlsReach,
    settings_pages::sheet_layout::{
        HEADER_PADDING, HeaderInset, SheetFrame, header_inset, sheet_frame,
    },
};

#[test]
fn sheet_keeps_position_when_it_fits() {
    // 13-inch, landscape: the canvas rectangle (Settings.dc.html).
    assert_eq!(
        sheet_frame(1376., 1032.),
        SheetFrame {
            left: 188.,
            top: 136.,
            width: 1000.,
            height: 760.,
            stacked: false
        }
    );
    // 136 + 760 + 16 is the smallest height that keeps the position.
    assert_eq!(sheet_frame(1376., 912.).top, 136.);
    let moved = sheet_frame(1376., 911.);
    assert_eq!((moved.top, moved.height), (16., 760.));
    // 11-inch with the software keyboard: top 16, shorter, content scrolls.
    let keyboard = sheet_frame(1210., 500.);
    assert_eq!((keyboard.top, keyboard.height), (16., 468.));
    assert!(keyboard.top + keyboard.height + 16. <= 500.);
}

#[test]
fn narrow_sheet_stacks_sections() {
    // Split View half (artboard 11, 605 x 834).
    let half = sheet_frame(605., 834.);
    assert!(half.stacked);
    assert_eq!((half.left, half.width), (16., 573.));
    // The sheet is 760 pt wide at a 792 pt window: two columns.
    assert!(sheet_frame(791., 834.).stacked);
    assert!(!sheet_frame(792., 834.).stacked);
    assert!(!sheet_frame(1210., 834.).stacked);
}

/// The shell's clearance for herdr-uikit's (left, right) sample.
fn controls(left: f32, right: f32) -> crate::layout::ControlsClearance {
    ControlsReach { left, right }.clearance()
}

const OWN: HeaderInset = HeaderInset {
    leading: HEADER_PADDING,
    trailing: HEADER_PADDING,
};

#[test]
fn a_sheet_at_the_top_keeps_its_header_past_the_window_controls() {
    // The narrow window of the PR 87 check (411 x 613 pt): the sheet sits
    // 16 pt from the top, inside the controls' band. Samples: iPadOS 26.5
    // Simulator, iPad Pro 13-inch (M5).
    let frame = sheet_frame(411., 613.);
    assert_eq!((frame.left, frame.top, frame.width), (16., 16., 379.));
    // Left: the header starts at the reach (66 pt) from the safe-area edge,
    // as the full sidebar's header does.
    let left = header_inset(frame, 411., controls(66., 9.5));
    assert_eq!(
        left,
        HeaderInset {
            leading: 50.,
            ..OWN
        }
    );
    assert_eq!(frame.left + left.leading, 66.);
    // Right (right-to-left): Done ends at the reach from the right edge, as
    // the tab row's pinned buttons do.
    let right = header_inset(frame, 411., controls(9., 66.));
    assert_eq!(
        right,
        HeaderInset {
            trailing: 50.,
            ..OWN
        }
    );
    assert_eq!(411. - (frame.left + frame.width) + right.trailing, 66.);
}

#[test]
fn no_controls_or_a_sheet_below_their_band_keeps_the_header_padding() {
    let top = sheet_frame(411., 613.);
    // All zero is no controls; a full-screen corner allowance is not them.
    assert_eq!(header_inset(top, 411., controls(0., 0.)), OWN);
    assert_eq!(header_inset(top, 411., controls(9.5, 9.)), OWN);
    // The canvas position (136 pt down) is below the controls' band.
    let canvas = sheet_frame(1376., 1032.);
    assert_eq!(canvas.top, 136.);
    assert_eq!(header_inset(canvas, 1376., controls(66., 9.5)), OWN);
    // A centred sheet whose edge is already past the reach adds nothing.
    let wide = sheet_frame(1210., 834.);
    assert_eq!(wide.top, 16.);
    assert!(wide.left >= 66.);
    assert_eq!(header_inset(wide, 1210., controls(66., 9.5)), OWN);
}
