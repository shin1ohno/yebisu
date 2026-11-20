//! One sidebar list for Herdr 1 and Herdr 2 (MIX-01, H2-07; design fidelity
//! plan, Wave 0a). Owner: stream A; stream F supplies the Herdr 2 entries.
//!
//! The sidebar shows one row per host, whichever Herdrs answer there, and
//! under it the host's Herdr 1 spaces, then its Herdr 2 sessions
//! (SidebarMixed.dc.html:28-53). Each connection still builds its own
//! `HostEntry` (Herdr 1 in `sidebar::model`, Herdr 2 in
//! `herdr2_frame::session`); [`merge`] folds them into rows. AGENTS stays one
//! list across every host and both Herdrs (`model::agents_list`).
use super::{
    HerdrKind,
    sidebar::model::{HostEntry, HostLink, SpaceEntry},
};
use crate::hosts::HostName;

/// One connection of a host: the Herdr that answers there and its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostConnection {
    pub(crate) kind: HerdrKind,
    pub(crate) link: HostLink,
}

/// A host row: its name and every connection the app holds to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListHost {
    pub(crate) name: HostName,
    /// At most one per kind, Herdr 1 first. The row's second line and dot
    /// come from these (SidebarMixed.dc.html:32 `SSH · Herdr 1 · Herdr 2`).
    pub(crate) connections: Vec<HostConnection>,
    /// The host of the terminal on screen.
    pub(crate) selected: bool,
}

/// A space row: a Herdr 1 space or a Herdr 2 session, with the connection
/// that owns it, so a tap opens it on that Herdr and no other.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ListSpace {
    /// The host's destination (`HostName::destination`).
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) entry: SpaceEntry,
}

/// One row of the merged list, in drawing order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ListRow {
    Host(ListHost),
    Space(ListSpace),
}

/// Folds the per-connection entries, given in Hosts order
/// (`model::order_hosts`), into the list. A host's row stands where its first
/// entry stands; its Herdr 1 spaces follow, then its Herdr 2 sessions. A
/// connection that is not connected adds its state to the host row and no
/// spaces (`HostEntry::with_content` keeps none then).
pub(crate) fn merge(hosts: &[HostEntry]) -> Vec<ListRow> {
    let mut groups: Vec<(&str, Vec<&HostEntry>)> = Vec::new();
    for entry in hosts {
        match groups.iter_mut().find(|(host, _)| *host == entry.host) {
            Some((_, entries)) => entries.push(entry),
            None => groups.push((entry.host.as_str(), vec![entry])),
        }
    }
    let mut rows = Vec::new();
    for (host, mut entries) in groups {
        // Stable: of two entries of one kind, the first one given counts.
        entries.sort_by_key(|entry| entry.kind);
        entries.dedup_by_key(|entry| entry.kind);
        rows.push(ListRow::Host(ListHost {
            name: HostName::new(host),
            connections: entries
                .iter()
                .map(|entry| HostConnection {
                    kind: entry.kind,
                    link: entry.link,
                })
                .collect(),
            selected: entries.iter().any(|entry| entry.selected),
        }));
        rows.extend(entries.iter().flat_map(|entry| {
            entry.spaces.iter().map(|space| {
                ListRow::Space(ListSpace {
                    host: host.to_owned(),
                    kind: entry.kind,
                    entry: space.clone(),
                })
            })
        }));
    }
    rows
}

#[cfg(test)]
#[path = "sidebar_list_tests.rs"]
mod tests;
