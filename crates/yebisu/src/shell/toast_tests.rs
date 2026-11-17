use super::*;
use crate::layout::{self, Insets, Rect, Size, TOAST_SIZE};

/// The toast's rect as `notification_view` places it: right-aligned,
/// `overlay_bottom` (= the keyboard) plus `toast_lift` above the bottom edge.
fn toast_rect(window: Size, keyboard: f32, lift: f32) -> Rect {
    let bottom = window.height - keyboard - lift;
    Rect::new(
        window.width - TOAST_SIZE.width,
        bottom - TOAST_SIZE.height,
        TOAST_SIZE.width,
        TOAST_SIZE.height,
    )
}

#[test]
fn notification_toast_clears_status_bar() {
    for window in [Size::new(1376.0, 1032.0), Size::new(605.0, 834.0)] {
        for (keyboard, safe_bottom) in [(0.0, 20.0), (320.0, 20.0), (0.0, 0.0), (10.0, 20.0)] {
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
            let lift = toast_lift_points(keyboard, safe_bottom, None);
            let toast = toast_rect(window, keyboard, lift);
            // Clear of the status bar and of its hairline above it.
            let bar = Rect::new(
                layout.status_bar.x,
                layout.status_bar.y - RULE,
                layout.status_bar.width,
                layout.status_bar.height + RULE,
            );
            assert!(!toast.overlaps(bar), "{window:?} {keyboard} {safe_bottom}");
            assert!(
                toast.bottom() <= bar.y,
                "{window:?} {keyboard} {safe_bottom}"
            );
        }
    }
    // L9's slot, when higher, wins; a lower slot never drops the toast onto
    // the status bar.
    assert_eq!(toast_lift_points(0.0, 20.0, Some((200.0, 340.0))), 200.0);
    assert_eq!(
        toast_lift_points(0.0, 20.0, Some((10.0, 340.0))),
        20.0 + STATUS_BAR_HEIGHT + RULE + TOAST_GAP
    );
}
