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
