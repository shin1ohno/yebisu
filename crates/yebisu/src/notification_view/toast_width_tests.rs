//! L9 review S2: the notification band takes overlay O's toast width on the
//! real screen, 340 shrunk to the window width − 32 (`overlay::toast_width`
//! through `render_notification`).
use super::Toast;
use crate::{
    app::Smoke,
    layout::{OVERLAY_MARGIN, TOAST_SIZE},
};
use gpui::{TestAppContext, px, size};
use std::time::{Duration, Instant};

/// Long enough that the band wraps at its width limit, not at its text.
const TITLE: &str = "Agent review-docs on pro-dev needs you: approve the plan before it runs the \
                     migration and the long test suite";

fn band_width(cx: &mut TestAppContext, window_width: f32) -> f32 {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(window_width), px(900.)));
    view.update(cx, |this, cx| {
        this.notifications.toast = Some(Toast {
            tag: "toast-width".into(),
            title: TITLE,
            expires: Instant::now() + Duration::from_secs(60),
        });
        cx.notify();
    });
    cx.run_until_parked();
    let band = cx
        .debug_bounds("notification-panel")
        .unwrap_or_else(|| panic!("notification band not painted at {window_width}"));
    f32::from(band.size.width)
}

#[gpui::test]
fn toast_width_is_340_in_a_wide_window(cx: &mut TestAppContext) {
    let width = band_width(cx, 1200.);
    assert!(
        (width - TOAST_SIZE.width).abs() < 0.5,
        "band {width} in a 1200 window"
    );
}

#[gpui::test]
fn toast_width_shrinks_to_window_minus_32(cx: &mut TestAppContext) {
    let width = band_width(cx, 300.);
    assert!(
        width <= 300. - OVERLAY_MARGIN + 0.5,
        "band {width} overflows a 300 window"
    );
}
