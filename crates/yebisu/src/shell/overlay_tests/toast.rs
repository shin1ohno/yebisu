//! The notification band sits in overlay O's toast slot, clear of the status
//! bar (S-2).
use super::super::*;
use crate::{
    layout::{self, Insets, Rect, Size},
    shell::toast::toast_lift_points,
};

/// The toast's rect as `notification_view` places it: right-aligned,
/// `overlay_bottom` plus `toast_lift` above the window's bottom edge.
fn placed_toast(window: Size, keyboard: f32, safe_bottom: f32) -> (Rect, Rect) {
    let layout = layout::compute(&layout::Input {
        window,
        insets: Insets {
            keyboard,
            safe_bottom,
            ..Insets::default()
        },
        pane_count: 1,
        ..layout::Input::default()
    });
    let overlay = keyboard;
    let lift = toast_lift_points(window.height, layout.status_bar.y, overlay, toast_slot());
    let width = toast_width(window.width);
    let bottom = window.height - overlay - lift;
    let toast = Rect::new(
        window.width - width,
        bottom - TOAST_SIZE.height,
        width,
        TOAST_SIZE.height,
    );
    let bar = Rect::new(
        layout.status_bar.x,
        layout.status_bar.y - RULE,
        layout.status_bar.width,
        layout.status_bar.height + RULE,
    );
    (toast, bar)
}

#[test]
fn notification_toast_uses_overlay_slot() {
    let (bottom, width) = toast_slot().unwrap_or_else(|| panic!("O places the toast"));
    assert_eq!(width, TOAST_SIZE.width);
    assert_eq!(bottom, STATUS_BAR_HEIGHT + RULE + TOAST_MARGIN);
    // With nothing under the status bar, the toast's bottom edge is the
    // slot's: O's margin above the status bar's hairline.
    let window = Size::new(1376.0, 1032.0);
    let (toast, bar) = placed_toast(window, 0.0, 0.0);
    assert_eq!(window.height - toast.bottom(), bottom);
    assert_eq!(bar.y - toast.bottom(), TOAST_MARGIN);
    // The width shrinks to the window width − 32.
    assert_eq!(toast_width(1376.0), 340.0);
    assert_eq!(toast_width(300.0), 268.0);
    assert_eq!(toast_width(10.0), 0.0);
}

#[test]
fn toast_never_covers_status_bar() {
    for window in [
        Size::new(1376.0, 1032.0),
        Size::new(1210.0, 834.0),
        Size::new(605.0, 834.0),
        Size::new(320.0, 480.0),
    ] {
        for keyboard in [0.0, 10.0, 320.0, 400.0] {
            for safe_bottom in [0.0, 20.0] {
                let (toast, bar) = placed_toast(window, keyboard, safe_bottom);
                let case = format!("{window:?} kb {keyboard} safe {safe_bottom}");
                assert!(!toast.overlaps(bar), "{case}");
                assert!(toast.bottom() <= bar.y, "{case}");
                // In O the slot keeps its own margin whenever it governs.
                if keyboard == 0.0 && safe_bottom == 0.0 {
                    assert!(bar.y - toast.bottom() >= TOAST_MARGIN, "{case}");
                }
            }
        }
    }
}
