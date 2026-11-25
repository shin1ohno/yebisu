//! The session's events in order: the query, Clear, the dismissals and the
//! filter each leaves (issue AC5, AC6, AC7, AC8, AC10, AC11).
use super::*;
use crate::shell::search::{DismissReason, Restored};

/// Typing in the field sets the sidebar filter and narrows the list behind.
#[gpui::test]
fn query_sets_filter_and_sidebar(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    assert!(
        cx.debug_bounds("sidebar-space-A-w2").is_some(),
        "fixture: space second is listed"
    );
    opened(&view, &fake, cx);
    push(&fake, cx, SearchEvent::QueryChanged("repo".into()));
    assert_eq!(filter(&view, cx), "repo");
    assert!(cx.debug_bounds("sidebar-space-A-w1").is_some());
    assert!(
        cx.debug_bounds("sidebar-space-A-w2").is_none(),
        "the sidebar did not narrow"
    );
    let (revision, shown) = fake.shown();
    assert_eq!(revision, 1, "the results were not updated");
    let titles: Vec<_> = shown
        .sections
        .iter()
        .flat_map(|s| s.items.iter().map(|i| i.title.as_str()))
        .collect();
    assert!(titles.contains(&"repo") && !titles.contains(&"second"));
}

/// Return closes the panel and keeps the filter; after the text view is
/// first responder again, one key reaches the terminal once.
#[gpui::test]
fn return_keeps_filter_next_key_once(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    push(&fake, cx, SearchEvent::QueryChanged("repo".into()));
    push(&fake, cx, SearchEvent::Dismissed(DismissReason::Return));
    assert!(fake.dropped(), "the handle was kept after the dismissal");
    assert_eq!(phase(&view, cx), Some(Phase::Restoring));
    push(&fake, cx, SearchEvent::InputRestored(Restored::Text));
    assert!(
        phase(&view, cx).is_none(),
        "the session outlived InputRestored"
    );
    assert_eq!(filter(&view, cx), "repo");
    typed_once(&view, &receipts, cx, "w1:p1", "x");
}

/// Escape (not composing; UIKit cancels a composition itself) clears it.
#[gpui::test]
fn escape_clears_filter(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    push(&fake, cx, SearchEvent::QueryChanged("repo".into()));
    close(&fake, cx, DismissReason::Escape, Restored::Text);
    assert_eq!(filter(&view, cx), "", "Escape kept the filter");
}

/// Every other way the panel closes keeps the filter.
#[gpui::test]
fn user_dismissals_keep_filter(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    for reason in [
        DismissReason::Cancel,
        DismissReason::OutsideTap,
        DismissReason::SwipedDown,
        DismissReason::Requested,
        DismissReason::System,
        DismissReason::WindowClosed,
    ] {
        view.update(cx, |this, _| this.clear_sidebar_filter());
        opened(&view, &fake, cx);
        push(&fake, cx, SearchEvent::QueryChanged("repo".into()));
        close(&fake, cx, reason, Restored::Text);
        assert_eq!(filter(&view, cx), "repo", "{reason:?} cleared the filter");
        assert!(phase(&view, cx).is_none());
    }
}

/// The SDK sends the final committed text before `Dismissed`: the filter is
/// that text, not an earlier one.
#[gpui::test]
fn final_query_before_dismissed(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    fake.push(SearchEvent::QueryChanged("rep".into()));
    fake.push(SearchEvent::QueryChanged("repo".into()));
    fake.push(SearchEvent::Dismissed(DismissReason::Return));
    cx.run_until_parked();
    assert_eq!(filter(&view, cx), "repo");
}

/// A burst settles on the latest query, each update's revision is newer
/// than the last, and the results follow the last query.
#[gpui::test]
fn burst_settles_on_latest(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    for n in 0..50 {
        fake.push(SearchEvent::QueryChanged(format!("q{n}")));
    }
    fake.push(SearchEvent::QueryChanged("second".into()));
    cx.run_until_parked();
    assert_eq!(filter(&view, cx), "second");
    let revisions: Vec<u64> = fake.0.borrow().updates.iter().map(|(r, _)| *r).collect();
    assert!(
        revisions.windows(2).all(|pair| pair[0] < pair[1]),
        "revisions out of order: {revisions:?}"
    );
    let (_, shown) = fake.shown();
    let titles: Vec<_> = shown
        .sections
        .iter()
        .flat_map(|s| s.items.iter().map(|i| i.title.as_str()))
        .collect();
    assert!(
        titles.contains(&"second") && !titles.contains(&"repo"),
        "the results do not follow the last query: {titles:?}"
    );
}

/// A selection then its dismissal are handled in that order.
#[gpui::test]
fn selected_then_dismissed_order(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let (revision, id) = row_id(&fake, "second");
    fake.push(SearchEvent::Selected { id, revision });
    fake.push(SearchEvent::Dismissed(DismissReason::Selected(id)));
    cx.run_until_parked();
    let state = fake.0.borrow();
    assert_eq!(state.dismissed, [id], "the selection was not opened first");
    assert_eq!(
        state.dropped_after,
        Some(1),
        "the handle was dropped before the selection's dismissal"
    );
}

/// "Clear filter" in the empty state empties the filter, the field (the
/// SDK's `set_query`) and the results together.
#[gpui::test]
fn clear_requested_empties_field(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    push(&fake, cx, SearchEvent::QueryChanged("no-such-space".into()));
    assert!(fake.shown().1.empty.is_some(), "fixture: the empty state");
    push(&fake, cx, SearchEvent::ClearRequested);
    assert_eq!(filter(&view, cx), "");
    assert_eq!(
        fake.0.borrow().queries,
        [String::new()],
        "the field kept its text"
    );
    let (_, shown) = fake.shown();
    assert!(shown.empty.is_none(), "the results were not rebuilt");
    assert_eq!(shown.status.as_deref(), Some(crate::shell::search::HINT));
}
