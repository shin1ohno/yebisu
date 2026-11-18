//! L2b tests: the QA strip (plan v2 5.3, 7.6).
use super::*;
use crate::shell::entry_move::LegacyBands;
use gpui::TestAppContext;

/// The strip exists only in a Simulator build with a QA variable set; the
/// host build never assembles it, and the old QA bands are retired.
#[gpui::test]
fn qa_strip_absent_without_qa_env(cx: &mut TestAppContext) {
    assert!(!strip_allowed(true, |_| false), "no QA variable");
    assert!(!strip_allowed(false, |_| true), "not a Simulator build");
    assert!(strip_allowed(true, |name| name == "YEBISU_HANDOFF_QA"));
    for name in QA_ENV {
        assert!(strip_allowed(true, |n| n == name), "{name}");
    }
    const { assert!(!STRIP_BUILD, "host tests are not a Simulator build") };
    assert!(!qa_strip_enabled());
    assert!(!LegacyBands::current().qa_controls);
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        assert!(this.render_qa_strip(cx).is_none());
    });
}

/// Integration 2: a press on the QA strip focuses the terminal at mouse down,
/// as the QA bands did inside the terminal body (`track_focus`), but never
/// over a sheet and never with the terminal closed.
#[gpui::test]
fn qa_strip_press_focuses_the_terminal_first(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            window.blur(cx);
            this.qa_strip_press(window, cx);
            assert!(
                !this.terminal_focus.is_focused(window),
                "terminal closed: no focus"
            );
            this.terminal.open = true;
            this.settings_modal = true;
            this.qa_strip_press(window, cx);
            assert!(
                !this.terminal_focus.is_focused(window),
                "a sheet keeps the keys"
            );
            this.settings_modal = false;
            this.qa_strip_press(window, cx);
            assert!(
                this.terminal_focus.is_focused(window),
                "the press focuses the terminal"
            );
        })
    });
}
