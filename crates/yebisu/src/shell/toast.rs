//! L2a: keeps the notification toast clear of the status bar (S-2).
//! Owner: L2a. L9 supplies the overlay slot through `overlay::toast_slot`.
//!
//! `notification_view` places the toast `overlay_bottom` (the software
//! keyboard height, or 0) above the window's bottom edge. The status bar sits
//! on the larger of the keyboard and the safe area bottom (D1) and is
//! `STATUS_BAR_HEIGHT` tall with a hairline over it, so the toast is lifted
//! past all of that.
use super::overlay;
use crate::{
    app::Smoke,
    layout::{RULE, STATUS_BAR_HEIGHT},
};
use gpui::{Pixels, Window, px};

/// Gap between the status bar's hairline and the toast.
pub(crate) const TOAST_GAP: f32 = 8.0;

/// Points to add to `overlay_bottom` (`ime_bottom`) so the toast's bottom edge
/// sits above the status bar. `slot` is L9's overlay placement as (bottom
/// offset from the window's bottom edge, width); it wins when it is higher.
pub(crate) fn toast_lift_points(
    ime_bottom: f32,
    safe_bottom: f32,
    slot: Option<(f32, f32)>,
) -> f32 {
    let ime = ime_bottom.max(0.0);
    let floor = ime.max(safe_bottom.max(0.0)) + STATUS_BAR_HEIGHT + RULE + TOAST_GAP;
    let wanted = slot.map_or(floor, |(bottom, _)| bottom.max(floor));
    (wanted - ime).max(0.0)
}

impl Smoke {
    /// Extra distance above `notification_view::overlay_bottom`.
    pub(crate) fn toast_lift(&self, window: &Window) -> Pixels {
        let insets = window.insets();
        px(toast_lift_points(
            f32::from(insets.ime.bottom),
            f32::from(insets.safe_area.bottom),
            overlay::toast_slot(),
        ))
    }
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;
