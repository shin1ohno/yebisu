//! Search results: sections, status, the empty state, ids, and the row
//! table keeping the shown and the previous revision only.
use super::*;
use crate::shell::sidebar::view::SidebarTap;
use gpui::{AppContext, TestAppContext};

fn tap(id: &str) -> RowTarget {
    RowTarget::Herdr1(SidebarTap::Host(id.into()))
}

#[test]
fn table_keeps_the_shown_and_the_previous_revision() {
    let mut table = RowTable::default();
    table.replace(0, vec![(1, tap("a"))]);
    table.replace(1, vec![(2, tap("b"))]);
    assert_eq!(table.lookup(0, 1), Some(&tap("a")), "the previous list");
    assert_eq!(table.lookup(1, 2), Some(&tap("b")), "the shown list");
    assert_eq!(table.lookup(1, 1), None, "an id of another revision");
    table.replace(2, vec![(3, tap("c"))]);
    assert_eq!(table.lookup(0, 1), None, "an older list is forgotten");
    assert_eq!(table.revisions(), [2, 1]);
}

#[gpui::test]
fn build_lists_spaces_then_agents_with_fresh_ids(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        let mut table = RowTable::default();
        let (shown, targets) = build(this, "", &mut table);
        assert_eq!(
            shown.status.as_deref(),
            Some(HINT),
            "a blank query shows the hint"
        );
        assert!(
            shown.empty.is_none(),
            "a blank query is not an empty result"
        );
        let (again, more) = build(this, "", &mut table);
        let ids: Vec<u64> = targets.iter().chain(&more).map(|(id, _)| *id).collect();
        assert!(
            ids.windows(2).all(|pair| pair[0] < pair[1]),
            "ids not monotonic: {ids:?}"
        );
        assert_eq!(shown.sections.len(), again.sections.len());

        let (none, nothing) = build(this, "no-such-space", &mut table);
        assert!(nothing.is_empty());
        assert!(none.sections.is_empty());
        let empty = none.empty.unwrap_or_else(|| panic!("no empty state"));
        assert_eq!(empty.message, "No matches for “no-such-space”");
        assert_eq!(empty.clear_label, "Clear filter");
        assert_eq!(none.status.as_deref(), Some("0 spaces · 0 agents"));
    });
}

#[test]
fn counts_are_singular_or_plural() {
    assert_eq!(plural(0, "space", "spaces"), "0 spaces");
    assert_eq!(plural(1, "space", "spaces"), "1 space");
    assert_eq!(plural(3, "agent", "agents"), "3 agents");
}
