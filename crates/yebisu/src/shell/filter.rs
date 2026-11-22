//! L4: the sidebar filter. Owner: L4.
//!
//! The magnifier narrows spaces and agents by a query. This file holds the
//! matching rule, the empty-result outcome and the query kept in
//! `RailState`; the sidebar list (L3) calls them.
//!
//! The field in the sidebar header takes no keyboard focus and no keys (owner
//! decision 2026-11-19, `pr77-filter-no-input-decision.json`): four review
//! rounds (G1, H1, SF1/S1, F1) each found a way for keys typed into a GPUI
//! field to reach the remote shell, the last one through UIKit composition of
//! a Japanese input source. It shows that filtering is not available yet,
//! and terminal input is exactly what it was without the field. TODO.md
//! tracks rebuilding it as a UIKit text-input target with IME support.
use super::sidebar::{AgentRow, model::AgentLine};
use crate::app::Smoke;
use crate::shell::style::{color, metric};
use gpui::{AnyElement, Context, Role, div, prelude::*, px, rgb};

/// Label of the button that clears an active filter.
pub(crate) const CLEAR_LABEL: &str = "Clear filter";
/// The search control's spoken name while filtering is unavailable.
pub(crate) const FILTER_UNAVAILABLE_LABEL: &str = "Search spaces and agents, not available yet";

/// The AGENTS lines a query leaves, in their order (`filter_rows`).
pub(crate) fn filter_lines(lines: Vec<AgentLine>, query: &str) -> Vec<AgentLine> {
    let rows: Vec<AgentRow> = lines.iter().map(|line| line.row.clone()).collect();
    let kept: Vec<&AgentRow> = filter_rows(&rows, query);
    lines
        .into_iter()
        .filter(|line| kept.iter().any(|row| row.identity == line.row.identity))
        .collect()
}

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

    /// The filter field in the sidebar header: shown as not available yet,
    /// with its reason. It has no focus handle, no key or mouse handler and
    /// registers no text input, so neither a tap nor a key changes terminal
    /// input or focus.
    pub(crate) fn render_filter_field(&self) -> AnyElement {
        super::sidebar::icons::Icon::Search
            .button(
                "sidebar-filter-unavailable",
                FILTER_UNAVAILABLE_LABEL,
                false,
            )
            .into_any_element()
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
                .px(px(metric::INSET))
                .flex()
                .items_center()
                .text_color(rgb(color::ACCENT))
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
