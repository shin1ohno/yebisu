use crate::{app::Smoke, input::Input};
use gpui::{EntityInputHandler, TestAppContext};

#[gpui::test]
fn modal_rejects_native_commits_composition_actions_and_clipboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.open = true;
            this.terminal_focus.focus(window, cx);
            this.terminal.input.composition = Input {
                text: "old".into(),
                marked: Some(0..3),
                selected: 3..3,
            };
            this.begin_settings_modal(window, cx);
            assert!(this.settings_modal);
            assert!(this.terminal.input.composition.text.is_empty());
            assert!(!this.accepts_text_input(window, cx));
            this.replace_text_in_range(None, "must not send", window, cx);
            this.replace_and_mark_text_in_range(None, "candidate", Some(0..1), window, cx);
            this.start_paste(false, cx);
            this.copy_selection(cx);
            assert_eq!(this.handoff.clipboard_reads, 0);
            assert_eq!(this.handoff.clipboard_writes, 0);
            assert!(
                this.run_action(sheprd_commands::Command::ToggleSidebar)
                    .is_err()
            );
            assert!(this.terminal.input.composition.text.is_empty());
            assert!(this.terminal.input.next().is_none());
            // A disappeared connection must restore the safe setup focus, not old terminal focus.
            this.terminal.open = false;
            this.end_settings_modal(window, cx);
            assert!(!this.settings_modal);
            assert!(this.focus.is_focused(window));
        });
    });
}
