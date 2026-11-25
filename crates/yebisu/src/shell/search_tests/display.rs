//! The filter after the panel closed: the magnifier's color and name, the
//! "Filtered by" row and its Clear button in both layouts (issue AC21).
use super::*;
use crate::shell::{
    filter::{SEARCH_LABEL, magnifier_ink},
    search::{DismissReason, Restored, search_label},
    style::color,
};

#[test]
fn magnifier_accent_and_label() {
    assert_eq!(magnifier_ink(""), color::MUTED);
    assert_eq!(magnifier_ink("  "), color::MUTED);
    assert_eq!(magnifier_ink("repo"), color::ACCENT);
    assert_eq!(SEARCH_LABEL, "Search spaces and agents");
    assert_eq!(search_label(""), "Search spaces and agents");
    assert_eq!(
        search_label("repo"),
        "Search spaces and agents, filtered by repo"
    );
}

fn kept_filter(view: &Entity<Smoke>, fake: &RecordedSearch, cx: &mut VisualTestContext) {
    opened(view, fake, cx);
    push(fake, cx, SearchEvent::QueryChanged("repo".into()));
    close(fake, cx, DismissReason::Return, Restored::Text);
    assert_eq!(filter(view, cx), "repo", "fixture: the filter was kept");
}

#[gpui::test]
fn filtered_by_row_normal_and_compact(cx: &mut TestAppContext) {
    for width in [1200., 340.] {
        let (view, _receipts, cx) = terminal_at(cx, "A", width);
        let fake = install_fake(&view, cx);
        if width < 352. {
            let menu = cx
                .debug_bounds("shell-compact-menu")
                .unwrap_or_else(|| panic!("fixture: the compact menu button"));
            cx.simulate_click(menu.center(), Modifiers::none());
            cx.run_until_parked();
        }
        assert!(
            cx.debug_bounds("filter-banner").is_none(),
            "width {width}: a banner without a filter"
        );
        kept_filter(&view, &fake, cx);
        assert!(
            cx.debug_bounds("filter-banner").is_some(),
            "width {width}: no \"Filtered by\" row"
        );
        assert!(
            cx.debug_bounds("filter-clear").is_some(),
            "width {width}: no Clear button"
        );
    }
}

/// Clear works while the filter still has results.
#[gpui::test]
fn clear_with_results(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    kept_filter(&view, &fake, cx);
    assert!(
        cx.debug_bounds("sidebar-space-A-w1").is_some(),
        "fixture: the filter has results"
    );
    let clear = cx
        .debug_bounds("filter-clear")
        .unwrap_or_else(|| panic!("the Clear button is absent with results"));
    cx.simulate_click(clear.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(filter(&view, cx), "");
    assert!(cx.debug_bounds("filter-banner").is_none());
    assert!(cx.debug_bounds("sidebar-space-A-w2").is_some());
}
