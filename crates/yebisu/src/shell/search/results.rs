//! The search panel's results: the rows the sidebar list shows for the
//! query (`Smoke::sidebar_rows`), as SPACES and AGENTS, each with the target
//! it opens. A row's target is the one it was drawn with (host, boot ID and
//! link generation), so a tap on a row drawn before a reconnect is refused
//! as stale, as in the sidebar.
use super::{HINT, SearchEmpty, SearchItem, SearchResults, SearchSection};
use crate::{
    app::Smoke,
    shell::{
        HerdrKind, filter,
        herdr2_frame::sidebar::CoreRow,
        sidebar::{accessibility_name, view::SidebarTap},
        sidebar_list::ListRow,
    },
};
use std::collections::HashMap;

pub(crate) const SPACES: &str = "SPACES";
pub(crate) const AGENTS: &str = "AGENTS";

/// A Herdr 2 row's target (`CoreRow` derives only `Clone`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoreTarget {
    Space(String),
    Agent(String),
}

impl CoreTarget {
    pub(crate) fn core(&self) -> CoreRow {
        match self {
            Self::Space(session) => CoreRow::Space(session.clone()),
            Self::Agent(pane) => CoreRow::Agent(pane.clone()),
        }
    }
}

/// What a result opens, as the sidebar row it mirrors would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RowTarget {
    Herdr1(SidebarTap),
    Herdr2 {
        host: String,
        boot: String,
        generation: u64,
        row: CoreTarget,
    },
}

type Shown = (u64, HashMap<u64, RowTarget>);

/// `(revision, id) -> target` for the results on screen and the ones they
/// just replaced: a tap UIKit sent before it showed the new list resolves.
#[derive(Debug, Default)]
pub(crate) struct RowTable {
    /// Ids are numbered once per session and never reused.
    next_id: u64,
    current: Option<Shown>,
    previous: Option<Shown>,
}

impl RowTable {
    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// `revision`'s rows are on screen now; the ones before stay one more.
    pub(crate) fn replace(&mut self, revision: u64, rows: Vec<(u64, RowTarget)>) {
        let shown = (revision, rows.into_iter().collect());
        self.previous = self.current.replace(shown);
    }

    pub(crate) fn lookup(&self, revision: u64, id: u64) -> Option<&RowTarget> {
        [self.current.as_ref(), self.previous.as_ref()]
            .into_iter()
            .flatten()
            .find(|(shown, _)| *shown == revision)
            .and_then(|(_, rows)| rows.get(&id))
    }

    /// The revisions the table still resolves, newest first.
    #[cfg(test)]
    pub(crate) fn revisions(&self) -> Vec<u64> {
        [self.current.as_ref(), self.previous.as_ref()]
            .into_iter()
            .flatten()
            .map(|(revision, _)| *revision)
            .collect()
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The results for `query`, with fresh ids from `table`, and each id's target.
pub(crate) fn build(
    app: &mut Smoke,
    query: &str,
    table: &mut RowTable,
) -> (SearchResults, Vec<(u64, RowTarget)>) {
    let shown = app.sidebar_rows(query);
    let mut targets = Vec::new();
    let mut spaces = Vec::new();
    for row in &shown.rows {
        let ListRow::Space(space) = row else {
            continue;
        };
        let id = table.next_id();
        let host = app.setup.host_name(&space.host);
        let tabs = plural(space.entry.tabs, "tab", "tabs");
        let target = match space.kind {
            HerdrKind::Herdr1 => RowTarget::Herdr1(space.tap()),
            HerdrKind::Herdr2 => RowTarget::Herdr2 {
                host: space.host.clone(),
                boot: space.boot.clone(),
                generation: space.generation,
                row: CoreTarget::Space(space.entry.id.clone()),
            },
        };
        targets.push((id, target));
        spaces.push(SearchItem {
            id,
            title: space.entry.name.clone(),
            detail: format!("{host} · {tabs}"),
            dot: Some(space.entry.dot.dot_color()),
            accessibility_label: format!(
                "Open {}, {host}, {tabs}, {}",
                space.entry.name,
                space.entry.dot.label()
            ),
        });
    }
    let mut agents = Vec::new();
    for line in &shown.agents {
        let identity = &line.row.identity;
        let Some(entry) = shown
            .hosts
            .iter()
            .find(|h| h.host == identity.host && h.kind == identity.kind)
        else {
            continue;
        };
        let id = table.next_id();
        let host = app.setup.host_name(&identity.host);
        let target = match identity.kind {
            HerdrKind::Herdr1 => RowTarget::Herdr1(SidebarTap::Agent {
                host: identity.host.clone(),
                pane: identity.pane.clone(),
                boot: entry.boot.clone(),
                generation: entry.generation,
            }),
            HerdrKind::Herdr2 => RowTarget::Herdr2 {
                host: identity.host.clone(),
                boot: entry.boot.clone(),
                generation: entry.generation,
                row: CoreTarget::Agent(identity.pane.clone()),
            },
        };
        targets.push((id, target));
        agents.push(SearchItem {
            id,
            title: line.shown.clone(),
            detail: line.place(&host),
            dot: Some(line.row.status.dot_color()),
            accessibility_label: format!("Open {}", accessibility_name(&line.row, &host)),
        });
    }
    let blank = query.trim().is_empty();
    let status = if blank {
        HINT.to_owned()
    } else {
        format!(
            "{} · {}",
            plural(spaces.len(), "space", "spaces"),
            plural(agents.len(), "agent", "agents")
        )
    };
    let empty = (!blank && spaces.is_empty() && agents.is_empty()).then(|| SearchEmpty {
        message: format!("No matches for “{}”", query.trim()),
        clear_label: filter::CLEAR_LABEL.to_owned(),
    });
    let sections = [(SPACES, spaces), (AGENTS, agents)]
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(title, items)| SearchSection {
            title: title.to_owned(),
            items,
        })
        .collect();
    (
        SearchResults {
            status: Some(status),
            message: None,
            sections,
            empty,
        },
        targets,
    )
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
