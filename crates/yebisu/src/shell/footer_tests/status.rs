use super::*;
use crate::shell::{entry_move::LegacyBands, status_bar};

fn has_phase_prefix(text: &str) -> bool {
    // "P9 · ...", "P0", "P13 · ...": a work-phase label.
    text.split_whitespace().any(|word| {
        word.strip_prefix('P')
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    })
}

/// Plan v2 7.6: no "P9 ·" status. The only place that drew it was the
/// legacy toolbar row, which L2b retires; the status bar line has no phase.
#[gpui::test]
fn status_has_no_phase_prefix(cx: &mut TestAppContext) {
    assert!(has_phase_prefix("P9 · Connected"), "positive control");
    let (view, cx) = window(cx);
    view.update(cx, |this, _| {
        assert!(
            !this.legacy_bands().toolbar,
            "the toolbar row (\"P9 · status\") is still drawn"
        );
        assert_ne!(this.legacy_bands(), LegacyBands::before_l2b());
        for connected in [true, false] {
            let mut state = this.chrome_state();
            state.connected = connected;
            let model = status_bar::status_bar_model(&state, "user@pro-dev.home.local");
            let line = status_bar::status_line(&model);
            assert!(!has_phase_prefix(&line), "{line}");
            assert!(!has_phase_prefix(&model.accessibility_label));
        }
    });
}
