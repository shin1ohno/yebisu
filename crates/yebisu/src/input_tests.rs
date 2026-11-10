use super::Input;

#[test]
fn composition_replaces_utf16_surrogate_pairs_and_clears_marked_text() {
    let mut input = Input::default();
    input.replace(None, "A🦀B", false, None);
    input.replace(Some(1..3), "え", true, Some(1..1));
    assert_eq!(input.text, "AえB");
    assert_eq!(input.marked, Some(1..2));
    input.replace(None, "恵", false, None);
    assert_eq!(input.text, "A恵B");
    assert_eq!(input.marked, None);
    assert_eq!(input.selected, 2..2);
}

#[test]
fn out_of_bounds_utf16_ranges_are_clamped_without_slicing_a_character() {
    let mut input = Input::default();
    input.replace(None, "🦀", false, None);
    input.replace(Some(99..100), "!", false, None);
    assert_eq!(input.text, "🦀!");
    input.replace(Some(1..2), "x", false, None);
    assert_eq!(input.text, "🦀x!");
}
