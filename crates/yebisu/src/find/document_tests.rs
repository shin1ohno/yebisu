//! Production callback contract; UIKit query proof requires the native probe.
#![allow(clippy::unwrap_used)]
use crate::find::field::Field;
use gpui::{EntityInputHandler, TestAppContext};

#[gpui::test]
fn d6_document_length_includes_suffix_after_caret_and_partial_emoji_cut(cx: &mut TestAppContext) {
    let (field, cx) = cx.add_window_view(|_, cx| Field::new(cx));
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.focus.focus(window, cx);
            field.replace_text_in_range(None, "abcdef", window, cx);
            field.set_selected_text_range(3..3, window, cx);
            assert_eq!(
                field.text_length_utf16(window, cx),
                Some(6),
                "D6 document end omitted suffix after caret"
            );
            assert_eq!(
                field.selected_text_range(false, window, cx).unwrap().range,
                3..3
            );
            let mut actual = None;
            assert_eq!(
                field
                    .text_for_range(3..6, &mut actual, window, cx)
                    .as_deref(),
                Some("def")
            );
            assert_eq!(actual, Some(3..6));

            field.input.set_text("pre::post".into());
            field.replace_and_mark_text_in_range(Some(4..4), "変😀中", Some(1..3), window, cx);
            assert_eq!(field.input.marked, Some(4..8));
            assert_eq!(field.input.selected, 5..7);
            assert_eq!(
                field.text_length_utf16(window, cx),
                Some(13),
                "D6 marked document omitted suffix"
            );
            field.cut(window, cx);
            assert_eq!(field.input.text, "pre:変中:post");
            assert_eq!(field.input.marked, Some(4..6));
            assert_eq!(field.input.selected, 5..5);
            assert_eq!(
                field.text_length_utf16(window, cx),
                Some(11),
                "D6 Cut document omitted suffix"
            );
            assert_eq!(
                field
                    .text_for_range(6..11, &mut actual, window, cx)
                    .as_deref(),
                Some(":post")
            );
        });
    });
}
