//! D1: the sheet keeps the canvas position when it fits, otherwise sits
//! 16 pt from the top; below 760 pt of width the sections stack.
use crate::settings_pages::sheet_layout::{SheetFrame, sheet_frame};

#[test]
fn sheet_keeps_position_when_it_fits() {
    // 13-inch, landscape: the canvas rectangle (Settings.dc.html).
    assert_eq!(
        sheet_frame(1376., 1032.),
        SheetFrame {
            left: 188.,
            top: 136.,
            width: 1000.,
            height: 760.,
            stacked: false
        }
    );
    // 136 + 760 + 16 is the smallest height that keeps the position.
    assert_eq!(sheet_frame(1376., 912.).top, 136.);
    let moved = sheet_frame(1376., 911.);
    assert_eq!((moved.top, moved.height), (16., 760.));
    // 11-inch with the software keyboard: top 16, shorter, content scrolls.
    let keyboard = sheet_frame(1210., 500.);
    assert_eq!((keyboard.top, keyboard.height), (16., 468.));
    assert!(keyboard.top + keyboard.height + 16. <= 500.);
}

#[test]
fn narrow_sheet_stacks_sections() {
    // Split View half (artboard 11, 605 x 834).
    let half = sheet_frame(605., 834.);
    assert!(half.stacked);
    assert_eq!((half.left, half.width), (16., 573.));
    // The sheet is 760 pt wide at a 792 pt window: two columns.
    assert!(sheet_frame(791., 834.).stacked);
    assert!(!sheet_frame(792., 834.).stacked);
    assert!(!sheet_frame(1210., 834.).stacked);
}
