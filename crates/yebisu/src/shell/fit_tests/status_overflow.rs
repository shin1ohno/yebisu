//! The status bar's details order: Hardware keyboard, then M5 (design text).
use super::super::*;
use crate::layout::{M5_MAX_SECTIONS, M5_SECTION_WIDTH};

// The three sections of artboard 10.
const _: () = assert!(M5_MAX_SECTIONS == 3);

const M5_FULL: f32 = M5_SECTION_WIDTH * 3.0;

#[test]
fn status_bar_overflow_order() {
    // Wide: nothing moves.
    assert!(status_bar_overflow_with(1200.0, M5_FULL).is_empty());
    assert!(status_bar_overflow(HARDWARE_KEYBOARD_WIDTH).is_empty());
    // Artboard 10: the three M5 sections fit, Hardware keyboard does not, so
    // Hardware keyboard moves first.
    assert_eq!(
        status_bar_overflow_with(M5_FULL + STATUS_GAP + 100.0, M5_FULL),
        vec![StatusItem::HardwareKeyboard]
    );
    // Narrower: the M5 sections follow.
    assert_eq!(
        status_bar_overflow_with(100.0, M5_FULL),
        vec![StatusItem::HardwareKeyboard, StatusItem::ModSections]
    );
    // No mod runs: only Hardware keyboard can move.
    assert_eq!(
        status_bar_overflow(HARDWARE_KEYBOARD_WIDTH - 1.0),
        vec![StatusItem::HardwareKeyboard]
    );
    assert_eq!(status_bar_overflow(0.0), vec![StatusItem::HardwareKeyboard]);
    // Every width: what moved is a prefix of the order, and a narrower bar
    // never keeps an item a wider bar moved.
    let mut wider: Vec<StatusItem> = Vec::new();
    for width in (0..=1200).rev() {
        let moved = status_bar_overflow_with(width as f32, M5_FULL);
        assert_eq!(moved[..], OVERFLOW_ORDER[..moved.len()], "{width}");
        assert!(moved.len() >= wider.len(), "{width}");
        wider = moved;
    }
}
