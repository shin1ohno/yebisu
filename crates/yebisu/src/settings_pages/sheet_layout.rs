//! L1: where the settings sheet sits in the window (D1, plan v2 7.4).
//! The canvas places the 1000 x 760 sheet 136 pt below the top of the
//! 13-inch window. When that does not fit, the sheet moves to 16 pt from the
//! top and its content scrolls; below 760 pt of width the section list goes
//! above the page (one column).

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
