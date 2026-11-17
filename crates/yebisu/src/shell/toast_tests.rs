use super::*;
use crate::layout::{self, Insets, Rect, Size, TOAST_SIZE};

/// The toast's rect as `notification_view` places it: right-aligned,
/// `overlay_bottom` plus `toast_lift` above the window's bottom edge.
fn toast_rect(window: Size, overlay: f32, lift: f32) -> Rect {
    let bottom = window.height - overlay - lift;
    Rect::new(
        window.width - TOAST_SIZE.width,
        bottom - TOAST_SIZE.height,
        TOAST_SIZE.width,
        TOAST_SIZE.height,
    )
}

#[test]
fn notification_toast_clears_status_bar() {
    // (frame keyboard, safe bottom, overlay_bottom = UIKit's IME inset). The
    // last cases disagree on purpose (review S3): the frame's keyboard comes
    // from the fully visible bounds, the toast's base from the IME inset.
    let cases = [
        (0.0, 20.0, 0.0),
        (320.0, 20.0, 320.0),
        (0.0, 0.0, 0.0),
        (10.0, 20.0, 10.0),
        (300.0, 20.0, 0.0),
        (300.0, 20.0, 350.0),
    ];
    for window in [Size::new(1376.0, 1032.0), Size::new(605.0, 834.0)] {
        for (keyboard, safe_bottom, overlay) in cases {
            let insets = Insets {
                safe_bottom,
                keyboard,
                ..Insets::default()
            };
            let layout = layout::compute(&layout::Input {
                window,
                insets,
                ..layout::Input::default()
            });
            let lift = toast_lift_points(window.height, layout.status_bar.y, overlay, None);
            let toast = toast_rect(window, overlay, lift);
            // Clear of the status bar and of its hairline above it.
            let bar = Rect::new(
                layout.status_bar.x,
                layout.status_bar.y - RULE,
                layout.status_bar.width,
                layout.status_bar.height + RULE,
            );
            let case = format!("{window:?} {keyboard} {safe_bottom} {overlay}");
            assert!(!toast.overlaps(bar), "{case}");
            assert!(toast.bottom() <= bar.y, "{case}");
        }
    }
    // L9's slot, when higher, wins; a lower slot never drops the toast onto
    // the status bar.
    let top = 1032.0 - 20.0 - STATUS_BAR_HEIGHT;
    assert_eq!(
        toast_lift_points(1032.0, top, 0.0, Some((200.0, 340.0))),
        200.0
    );
    assert_eq!(
        toast_lift_points(1032.0, top, 0.0, Some((10.0, 340.0))),
        20.0 + STATUS_BAR_HEIGHT + RULE + TOAST_GAP
    );
}
