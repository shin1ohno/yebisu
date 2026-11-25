//! L4: the sidebar filter. Owner: L4.
//!
//! The magnifier narrows spaces and agents by a query. This file holds the
//! matching rule, the empty-result outcome and the query kept in
//! `RailState`; the sidebar list (L3) calls them.
//!
//! The magnifier in the sidebar header takes no keyboard focus and no keys
//! itself (owner decision 2026-11-19, `pr77-filter-no-input-decision.json`:
//! keys typed into a GPUI field reached the remote shell). It opens
//! herdr-uikit's native search panel (`shell::search`), whose UIKit field
//! owns keys, composition and Paste while it is up; the query it sends is
//! kept here and stays after the panel closed, shown by the magnifier's
//! color and the "Filtered by" row with its Clear button.
use super::sidebar::{AgentRow, model::AgentLine};
use crate::app::Smoke;
use crate::shell::style::{color, metric};
use gpui::{AnyElement, Context, Role, canvas, div, prelude::*, px, rgb};

/// Label of the button that clears an active filter.
pub(crate) const CLEAR_LABEL: &str = "Clear filter";
/// The magnifier's spoken name (`search::search_label` adds the filter).
pub(crate) const SEARCH_LABEL: &str = "Search spaces and agents";

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

/// The magnifier's color: the accent while a filter is kept.
pub(crate) fn magnifier_ink(query: &str) -> u32 {
    if query.trim().is_empty() {
        color::MUTED
    } else {
        color::ACCENT
    }
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

    /// The magnifier in the sidebar header (normal and compact): it opens
    /// the native search panel, anchored at its painted bounds. It has no
    /// focus handle and registers no text input; it is in the accent color
    /// while a filter is set.
    pub(crate) fn render_filter_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.sidebar_filter();
        let ink = magnifier_ink(query);
        let anchor = self.shell.search.anchor.clone();
        #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
        let qa = (super::search_qa::snapshot(self), self.observation.clone());
        super::sidebar::icons::Icon::Search
            .button_with_ink("sidebar-search", super::search::search_label(query), ink)
            .relative()
            .child(
                canvas(
                    move |bounds, _window, _| {
                        anchor.set(Some(bounds));
                        #[cfg(all(
                            target_os = "ios",
                            target_abi = "sim",
                            feature = "test-support"
                        ))]
                        super::search_qa::publish(_window.window_handle(), qa.0.clone(), &qa.1);
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .children(crate::qa_ax::mark(
                crate::qa_ax::ids::SIDEBAR_SEARCH,
                SEARCH_LABEL,
            ))
            .on_click(cx.listener(|this, event: &gpui::ClickEvent, window, cx| {
                let anchor = this.search_anchor(event.position());
                this.open_sidebar_search(anchor, window, cx);
            }))
            .into_any_element()
    }

    /// The row heading the list while a filter is set, with or without
    /// results: what it filters by and the button that clears it.
    pub(crate) fn render_filter_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let query = self.shell.rail.filter.trim().to_owned();
        if query.is_empty() {
            return None;
        }
        Some(
            div()
                .id("filter-banner")
                .debug_selector(|| "filter-banner".to_owned())
                .flex()
                .items_center()
                .justify_between()
                .gap(px(metric::INLINE_GAP))
                .pl(px(metric::INSET))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(color::MUTED))
                        .child(format!("Filtered by “{query}”")),
                )
                .children(self.render_filter_clear(cx))
                .into_any_element(),
        )
    }

    /// The button that clears the filter, in the "Filtered by" row. `None`
    /// while no filter is set.
    pub(crate) fn render_filter_clear(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.shell.rail.filter.trim().is_empty() {
            return None;
        }
        Some(
            div()
                .id("filter-clear")
                .debug_selector(|| "filter-clear".to_owned())
                .relative()
                .role(Role::Button)
                .aria_label(CLEAR_LABEL)
                .min_h(px(44.))
                .px(px(metric::INSET))
                .flex()
                .items_center()
                .text_color(rgb(color::ACCENT))
                .child(CLEAR_LABEL)
                .children(crate::qa_ax::mark(
                    crate::qa_ax::ids::SIDEBAR_FILTER_CLEAR,
                    CLEAR_LABEL,
                ))
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
