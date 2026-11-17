//! L2a: keeps the notification toast clear of the status bar (S-2).
//! Owner: L2a. L9 supplies the overlay slot through `overlay::toast_slot`.
use crate::app::Smoke;
use gpui::{Pixels, Window, px};

impl Smoke {
    /// Extra distance above `notification_view::overlay_bottom`.
    pub(crate) fn toast_lift(&self, _window: &Window) -> Pixels {
        px(0.)
    }
}
