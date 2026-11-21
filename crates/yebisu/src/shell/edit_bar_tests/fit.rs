//! Rule: four buttons share one row; when they do not fit, Paste image moves
//! to the `…` menu and nothing else does.
use crate::shell::edit_bar::{EditBarItem, edit_bar_fit, edit_bar_items};

#[test]
fn paste_image_moves_to_the_menu_when_the_row_is_narrow() {
    let wide = edit_bar_fit(true, 700.0);
    assert_eq!(wide.shown, edit_bar_items(true));
    assert!(wide.overflow.is_empty());
    let narrow = edit_bar_fit(true, 280.0);
    assert_eq!(
        narrow.shown,
        [
            EditBarItem::Copy,
            EditBarItem::CopyOneLine,
            EditBarItem::Paste,
            EditBarItem::Cancel
        ]
    );
    assert_eq!(narrow.overflow, [EditBarItem::PasteImage]);
    let none = edit_bar_fit(false, 700.0);
    assert!(none.shown.is_empty() && none.overflow.is_empty());
}

#[test]
fn the_row_breaks_at_the_width_of_four_buttons() {
    // 64 + 68 + 112 + 44 plus 3 dividers and a 10 px margin each side.
    assert!(edit_bar_fit(true, 311.0).overflow.is_empty());
    assert_eq!(edit_bar_fit(true, 310.9).overflow.len(), 1);
}
