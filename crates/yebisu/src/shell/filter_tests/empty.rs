use super::{filter_rows, fleet};
use crate::shell::filter::{CLEAR_LABEL, FilterOutcome, filter_outcome};

#[test]
fn filter_empty_offers_clear() {
    let rows = fleet();
    // A query that leaves nothing offers the way out.
    let left = filter_rows(&rows, "no-such-agent");
    assert!(left.is_empty());
    assert_eq!(
        filter_outcome("no-such-agent", left.len()),
        FilterOutcome::Empty {
            clear_label: "Clear filter"
        }
    );
    assert_eq!(CLEAR_LABEL, "Clear filter");
    // Results are not an empty state, and no query is not a filtered state.
    let left = filter_rows(&rows, "claude");
    assert_eq!(
        filter_outcome("claude", left.len()),
        FilterOutcome::Results(2)
    );
    assert_eq!(filter_outcome("", 0), FilterOutcome::Unfiltered);
    assert_eq!(filter_outcome("  ", 0), FilterOutcome::Unfiltered);
}
