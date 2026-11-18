//! L4: the sidebar filter. Owner: L4.
//!
//! The magnifier narrows spaces and agents by what the user types. This file
//! holds the matching rule, the empty-result outcome and the query kept in
//! `RailState`; the sidebar list (L3) calls them.
use super::sidebar::AgentRow;
use crate::app::Smoke;
use gpui::{AnyElement, Context, Role, div, prelude::*, px, rgb};

/// Label of the button that clears an active filter.
pub(crate) const CLEAR_LABEL: &str = "Clear filter";

/// True when every word of `query` appears in `text`, ignoring case. An empty
/// query matches everything.
pub(crate) fn matches(query: &str, text: &str) -> bool {
    let haystack = text.to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// The rows a query leaves, in their order. A row matches by its name or its
/// host, so `pro-dev` lists that host's agents.
pub(crate) fn filter_rows<'a>(rows: &'a [AgentRow], query: &str) -> Vec<&'a AgentRow> {
    rows.iter()
        .filter(|row| matches(query, &format!("{} {}", row.name, row.identity.host)))
        .collect()
}

/// What the list shows for a query and the number of rows it leaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FilterOutcome {
    /// No query: the list is whole (an empty list is L3's "New space" state).
    Unfiltered,
    Results(usize),
    /// The query left nothing: say so and offer the way out (D7).
    Empty {
        clear_label: &'static str,
    },
}

pub(crate) fn filter_outcome(query: &str, shown: usize) -> FilterOutcome {
    if query.trim().is_empty() {
        FilterOutcome::Unfiltered
    } else if shown == 0 {
        FilterOutcome::Empty {
            clear_label: CLEAR_LABEL,
        }
    } else {
        FilterOutcome::Results(shown)
    }
}

impl Smoke {
    pub(crate) fn sidebar_filter(&self) -> &str {
        &self.shell.rail.filter
    }

    pub(crate) fn set_sidebar_filter(&mut self, query: &str) {
        self.shell.rail.filter = query.to_owned();
    }

    pub(crate) fn clear_sidebar_filter(&mut self) {
        self.shell.rail.filter.clear();
    }

    /// The empty-result row: the button that clears the filter. `None` while
    /// no filter is set; the list shows it when `filter_outcome` is `Empty`.
    pub(crate) fn render_filter_clear(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.shell.rail.filter.trim().is_empty() {
            return None;
        }
        Some(
            div()
                .id("filter-clear")
                .debug_selector(|| "filter-clear".to_owned())
                .role(Role::Button)
                .aria_label(CLEAR_LABEL)
                .min_h(px(44.))
                .px_3()
                .flex()
                .items_center()
                .text_color(rgb(0x88c0d0))
                .child(CLEAR_LABEL)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.clear_sidebar_filter();
                    cx.notify();
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
