//! L1: where the settings sheet sits in the window (D1, plan v2 7.4).
//! The canvas places the 1000 x 760 sheet 136 pt below the top of the
//! 13-inch window. When that does not fit, the sheet moves to 16 pt from the
//! top and its content scrolls; below 760 pt of width the section list goes
//! above the page (one column).
//!
//! A sheet moved to the top reaches into the band iPadOS 26's window controls
//! cover (`layout/controls.rs`); its header then starts and ends where the
//! shell chrome does, past the controls' reach.
use crate::layout::{ControlsClearance, SIDEBAR_HEADER_HEIGHT};

/// The canvas size of the sheet.
pub(crate) const MAX_WIDTH: f32 = 1000.;
pub(crate) const MAX_HEIGHT: f32 = 760.;
/// The canvas distance from the window top (`Settings.dc.html`).
pub(crate) const CANVAS_TOP: f32 = 136.;
/// The margin kept on every side when the sheet does not fit.
pub(crate) const MARGIN: f32 = 16.;
/// Below this sheet width the sections stack above the page.
pub(crate) const STACK_BELOW: f32 = 760.;

/// The sheet's rectangle relative to the available area (safe area and
/// software keyboard removed), and whether the sections stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SheetFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub stacked: bool,
}

/// The Settings header's own side padding.
pub(crate) const HEADER_PADDING: f32 = crate::shell::style::metric::INLINE_GAP;

/// The Settings header's padding inside the sheet's left and right edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeaderInset {
    pub leading: f32,
    pub trailing: f32,
}

/// Where the header of the sheet in `frame` keeps its content: its own
/// padding, or, while the sheet's top lies in the window controls' band (the
/// height the rail keeps empty under them, `SIDEBAR_HEADER_HEIGHT`), the
/// shell's clearance `controls` measured from the sheet's edge. The reach is
/// from the safe area's edges, as `frame` is.
pub(crate) fn header_inset(
    frame: SheetFrame,
    available_width: f32,
    controls: ControlsClearance,
) -> HeaderInset {
    let mut inset = HeaderInset {
        leading: HEADER_PADDING,
        trailing: HEADER_PADDING,
    };
    if frame.top >= SIDEBAR_HEADER_HEIGHT {
        return inset;
    }
    // Controls on the left: the rail keeps the band empty and the sidebar
    // header starts past their reach.
    if controls.rail_top > 0. {
        inset.leading = inset.leading.max(controls.header_leading - frame.left);
    }
    let right_gap = available_width - frame.left - frame.width;
    inset.trailing = inset.trailing.max(controls.tab_row_trailing - right_gap);
    inset
}

pub(crate) fn sheet_frame(available_width: f32, available_height: f32) -> SheetFrame {
    let width = (available_width - 2. * MARGIN).clamp(0., MAX_WIDTH);
    let fits = CANVAS_TOP + MAX_HEIGHT + MARGIN <= available_height;
    let (top, height) = if fits {
        (CANVAS_TOP, MAX_HEIGHT)
    } else {
        (
            MARGIN,
            (available_height - 2. * MARGIN).clamp(0., MAX_HEIGHT),
        )
    };
    SheetFrame {
        left: ((available_width - width) / 2.).max(0.),
        top,
        width,
        height,
        stacked: width < STACK_BELOW,
    }
}
