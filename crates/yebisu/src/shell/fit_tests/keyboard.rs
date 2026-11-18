//! A floating keyboard never resizes the terminal; the edit bar and the IME
//! candidates move clear of it (design text, "浮いたキーボード").
use super::super::*;
use crate::layout::{self, Insets};

fn terminal_with_keyboard(window: Size, keyboard: Option<Rect>) -> Rect {
    let insets = Insets {
        keyboard: docked_keyboard_height(window, keyboard),
        ..Insets::default()
    };
    layout::compute(&layout::Input {
        window,
        insets,
        pane_count: 1,
        ..layout::Input::default()
    })
    .terminal
    .rect()
    .unwrap_or_else(|| panic!("{window:?} {keyboard:?} must fit"))
}

#[test]
fn floating_keyboard_does_not_resize_terminal() {
    let window = Size::new(1376.0, 1032.0);
    let without = terminal_with_keyboard(window, None);
    // Floating keyboards: small, anywhere, including touching the bottom
    // edge but not spanning the width.
    for floating in [
        Rect::new(500.0, 400.0, 320.0, 260.0),
        Rect::new(900.0, 772.0, 320.0, 260.0),
        Rect::new(0.0, 700.0, 600.0, 332.0),
    ] {
        assert_eq!(docked_keyboard_height(window, Some(floating)), 0.0);
        assert_eq!(terminal_with_keyboard(window, Some(floating)), without);
    }
    // A docked keyboard is an L0 inset and shortens the terminal.
    let docked = Rect::new(0.0, 1032.0 - 320.0, 1376.0, 320.0);
    assert_eq!(docked_keyboard_height(window, Some(docked)), 320.0);
    let with_docked = terminal_with_keyboard(window, Some(docked));
    assert_eq!(with_docked.width, without.width);
    assert!(with_docked.height < without.height);
    // 13-inch with the software keyboard keeps at least 8 rows (206 pt).
    assert!(with_docked.height >= TERMINAL_MIN_HEIGHT);

    // The edit bar moves above a floating keyboard that covers it, or below
    // when there is no room above, and is left alone otherwise.
    let column = without;
    let bar = Rect::new(column.x + 8.0, column.y + 8.0, 300.0, 44.0);
    let away = Rect::new(column.x + 600.0, column.y + 400.0, 320.0, 260.0);
    assert_eq!(place_clear_of_keyboard(bar, away, column), Some(bar));
    let over = Rect::new(column.x, column.y + 200.0, 400.0, 260.0);
    let low_bar = Rect::new(bar.x, column.y + 300.0, bar.width, bar.height);
    let moved = place_clear_of_keyboard(low_bar, over, column)
        .unwrap_or_else(|| panic!("room above the keyboard"));
    assert!(!moved.overlaps(over));
    assert_eq!(moved.bottom(), over.y);
    let at_top = Rect::new(column.x, column.y, 400.0, 260.0);
    let below = place_clear_of_keyboard(bar, at_top, column)
        .unwrap_or_else(|| panic!("room below the keyboard"));
    assert_eq!(below.y, at_top.bottom());
    // No room on either side: the actions go to the standard edit menu.
    let tall = Rect::new(column.x, column.y, 400.0, column.height);
    assert!(place_clear_of_keyboard(bar, tall, column).is_none());
}
