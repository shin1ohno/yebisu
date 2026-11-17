//! Rule: the status bar says why typing is paused.
use super::support::{selection, window};
use crate::shell::edit_bar::{paused_typing_status, selection_status_line};

const FULL: &str =
    "Selected 7 characters · ⌘C copies · Typing is paused until you cancel the selection";
const PAUSED: &str = "Typing is paused until you cancel the selection";

#[test]
fn status_explains_paused_typing() {
    assert_eq!(paused_typing_status(true), Some(PAUSED));
    assert_eq!(paused_typing_status(false), None);
    assert_eq!(selection_status_line(7, 200), FULL);
    assert_eq!(
        selection_status_line(1, 200),
        "Selected 1 character · ⌘C copies · Typing is paused until you cancel the selection"
    );
}

#[test]
fn a_long_selection_still_fits_one_line() {
    let chars = 1_234_567;
    let full = selection_status_line(chars, 200);
    assert!(full.starts_with("Selected 1234567 characters"));
    for width in [80, 60, 47, 30, 12, 1, 0] {
        let line = selection_status_line(chars, width);
        assert!(line.chars().count() <= width, "{width}: {line}");
        assert!(!line.contains('\n'));
    }
    // The pause explanation outlives the count and the shortcut hint.
    assert!(selection_status_line(chars, 60).ends_with(PAUSED));
    assert!(selection_status_line(chars, 47).ends_with(PAUSED));
    let cut = selection_status_line(chars, 30);
    assert!(cut.ends_with('…') && cut.starts_with("Typing is paused"));
}

#[gpui::test]
fn the_view_counts_the_selected_characters(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        assert_eq!(this.selection_status(200), None);
        this.handoff.selection = Some(selection());
        assert_eq!(
            this.selection_status(200).as_deref(),
            Some("Selected 3 characters · ⌘C copies · Typing is paused until you cancel the selection")
        );
    });
}
