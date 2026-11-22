//! L2a: keeps the notification toast clear of the status bar (S-2).
//! Owner: L2a. L9 supplies the overlay slot through `overlay::toast_slot`.
//!
//! `notification_view` places the toast `overlay_bottom` (the software
//! keyboard height, or 0) above the window's bottom edge. The status bar sits
//! on the larger of the keyboard and the safe area bottom (D1) and is
//! `STATUS_BAR_HEIGHT` tall with a hairline over it, so the toast is lifted
//! past all of that.
use super::overlay;
use crate::shell::style::metric;
use crate::{
    app::Smoke,
    layout::{RULE, STATUS_BAR_HEIGHT},
    notification_view::overlay_bottom,
};
use gpui::{Pixels, Window, px};

/// Gap between the status bar's hairline and the toast.
pub(crate) const TOAST_GAP: f32 = metric::INLINE_GAP;

/// Points to add to `overlay_bottom` so the toast's bottom edge sits above
/// the status bar and its hairline. The status bar's top comes from the same
/// L0 layout the frame draws (`status_bar_top`, measured from the window's
/// top in a window `window_height` tall), so the toast and the status bar
/// never use two different keyboard heights (review S3). `slot` is L9's
/// overlay placement as (bottom offset from the window's bottom edge,
/// width); it wins when it is higher.
pub(crate) fn toast_lift_points(
    window_height: f32,
    status_bar_top: f32,
    overlay_bottom: f32,
    slot: Option<(f32, f32)>,
) -> f32 {
    let base = overlay_bottom.max(0.0);
    let floor = (window_height - status_bar_top).max(STATUS_BAR_HEIGHT) + RULE + TOAST_GAP;
    let wanted = slot.map_or(floor, |(bottom, _)| bottom.max(floor));
    (wanted - base).max(0.0)
}

impl Smoke {
    /// Extra distance above `notification_view::overlay_bottom`.
    pub(crate) fn toast_lift(&self, window: &Window) -> Pixels {
        let layout = self.frame_layout(window);
        px(toast_lift_points(
            f32::from(window.viewport_size().height),
            layout.status_bar.y,
            f32::from(overlay_bottom(&window.insets())),
            overlay::toast_slot(),
        ))
    }
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;
