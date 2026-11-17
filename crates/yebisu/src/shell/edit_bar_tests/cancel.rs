//! Rule: Cancel ends the selection and typing resumes (the `input.rs` stop
//! lifts: `Handoff::blocks_input` and the text-input fence both clear).
use super::support::{selection, window};

#[gpui::test]
fn cancel_resumes_typing(cx: &mut gpui::TestAppContext) {
    for with_select_mode in [true, false] {
        let (view, cx) = window(cx);
        view.update(cx, |this, _| {
            this.handoff.select_mode = with_select_mode;
            this.handoff.selection = Some(selection());
            this.terminal.input.fence();
            assert!(this.is_selecting(), "typing is paused");
            assert!(this.terminal.input.requires_gesture(), "input is fenced");
        });
        cx.update(|window, cx| {
            view.update(cx, |this, cx| this.cancel_selection(window, cx));
        });
        view.update(cx, |this, _| {
            assert!(!this.is_selecting(), "select_mode {with_select_mode}");
            assert!(!this.handoff.select_mode);
            assert!(this.handoff.selection.is_none());
            assert!(
                !this.terminal.input.requires_gesture(),
                "Cancel is the gesture that lets typing resume"
            );
        });
    }
}

/// The state half, observed before any render: a render drops a selection
/// whose frame is gone, which would hide a Cancel that forgot to clear it.
#[gpui::test]
fn cancel_clears_the_selection_itself(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        this.handoff.selection = Some(selection());
        assert!(this.is_selecting());
        this.end_selection();
        assert!(this.handoff.selection.is_none());
        assert!(!this.is_selecting());
    });
}

#[gpui::test]
fn cancel_keeps_the_fence_while_find_has_typing_paused(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        this.handoff.select_mode = true;
        this.find_handoff.paused = true;
        this.terminal.input.fence();
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| this.cancel_selection(window, cx));
    });
    view.update(cx, |this, _| {
        assert!(!this.is_selecting());
        assert!(
            this.terminal.input.requires_gesture(),
            "find still owns typing, as with the Select toggle"
        );
    });
}
