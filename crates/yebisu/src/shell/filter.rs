//! L4: the sidebar filter. Owner: L4.
use super::sidebar::AgentRow;

pub(crate) fn filter_rows<'a>(rows: &'a [AgentRow], _query: &str) -> Vec<&'a AgentRow> {
    rows.iter().collect()
}
