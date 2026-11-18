//! L9: overlay O, including the toast slot (S-2). Owner: L9.
//!
//! O holds dialogs (420) and the toast (340×72) over everything but the
//! yebisu sheets. Its widths shrink to the window width − 32 (design text,
//! "O オーバーレイ"). The toast slot sits half that margin above the status
//! bar and its hairline, so the notification band (PR 45) never lies on the
//! status bar.
use crate::{
    app::Smoke,
    layout::{OVERLAY_MARGIN, RULE, STATUS_BAR_HEIGHT, TOAST_SIZE},
};
use gpui::{Pixels, Window, px};

/// Clear space between the status bar's hairline and the toast in O.
pub(crate) const TOAST_MARGIN: f32 = OVERLAY_MARGIN / 2.0;

/// The toast slot as (bottom offset, width) in points, when overlay O
/// places it; `None` keeps the L2a placement.
pub(crate) fn toast_slot() -> Option<(f32, f32)> {
    Some((STATUS_BAR_HEIGHT + RULE + TOAST_MARGIN, TOAST_SIZE.width))
}

/// The toast width in a window `window_width` wide: 340, shrunk to the
/// window width − 32.
pub(crate) fn toast_width(window_width: f32) -> f32 {
    TOAST_SIZE
        .width
        .min((window_width - OVERLAY_MARGIN).max(0.0))
}

impl Smoke {
    /// The notification band's width limit in this window (`toast_width`).
    pub(crate) fn toast_max_width(&self, window: &Window) -> Pixels {
        px(toast_width(f32::from(window.viewport_size().width)))
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
