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

#[gpui::test]
fn saved_target_and_delayed_load_never_change_p0_input(cx: &mut gpui::TestAppContext) {
    use crate::{app::Smoke, setup::Reply};
    use gpui::EntityInputHandler;
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.apply_setup_result(Ok(Reply::Loaded("user@saved".into(), "pin".into())));
            assert_eq!(this.setup_input.text, "user@saved");
            assert!(this.input.text.is_empty());
            this.smoke_mode = true;
            this.focus.focus(window, cx);
            this.replace_text_in_range(None, "P0-input", window, cx);
            assert_eq!(this.input.text, "P0-input");
            // This is the same completion adapter called by the background Load.
            this.apply_setup_result(Ok(Reply::Loaded("user@late".into(), "late pin".into())));
            assert_eq!(
                this.input.text, "P0-input",
                "late Load overwrote typed text"
            );
            assert_eq!(this.setup_input.text, "user@late");
            this.smoke_mode = false;
            this.replace_text_in_range(Some(0..9), "other@host", window, cx);
            assert_eq!(this.setup_input.text, "other@host");
            assert_eq!(this.input.text, "P0-input");
        });
    });
}
