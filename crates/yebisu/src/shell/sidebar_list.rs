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
    sidebar::{
        model::{HostEntry, HostLink, OfflineReason, SpaceEntry},
        view::SidebarTap,
    },
};
use crate::{hosts::HostName, setup::Setup};

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

impl ListHost {
    pub(super) fn link(&self) -> HostLink {
        if self
            .connections
            .iter()
            .any(|c| c.link == HostLink::Connected)
        {
            HostLink::Connected
        } else if self
            .connections
            .iter()
            .any(|c| c.link == HostLink::Connecting)
        {
            HostLink::Connecting
        } else {
            self.connections
                .iter()
                .map(|connection| connection.link)
                .find(|link| *link != HostLink::Offline(OfflineReason::NotConnected))
                .unwrap_or(HostLink::Offline(OfflineReason::NotConnected))
        }
    }

    pub(super) fn connection_detail(&self) -> String {
        let link = self.link();
        let mut detail = String::from("SSH");
        let mut answered = 0;
        for connection in &self.connections {
            if link == HostLink::Connected && connection.link != link {
                continue;
            }
            detail.push_str(match connection.kind {
                HerdrKind::Herdr1 => " · Herdr 1",
                HerdrKind::Herdr2 => " · Herdr 2",
            });
            answered += 1;
        }
        // SidebarMixed.dc.html:35 shows the two responding daemons together.
        if answered == 2 && link == HostLink::Connected {
            return detail;
        }
        let status = match link {
            HostLink::Connected => "Connected",
            HostLink::Connecting => "Connecting…",
            HostLink::Offline(reason) => reason.label(),
        };
        format!("{detail} · {status}")
    }
}

/// A space row: a Herdr 1 space or a Herdr 2 session, with the connection
/// that owns it, so a tap opens it on that Herdr and no other.
///
/// The row also keeps the connection it was drawn from: the daemon's boot ID
/// and the app's link generation, copied from its `HostEntry` when the list
/// is built. A tap sends these drawn values ([`Self::tap`]), never the ones
/// current at tap time, so a row drawn before a reconnect is rejected as
/// stale (`board::navigate_board`) even when the new connection has a space
/// with the same ID.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ListSpace {
    /// The host's destination (`HostName::destination`).
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    /// `HostEntry::boot` when the row was drawn.
    pub(crate) boot: String,
    /// `HostEntry::generation` when the row was drawn.
    pub(crate) generation: u64,
    pub(crate) entry: SpaceEntry,
}

impl ListSpace {
    /// The tap this row sends: its space on the connection it was drawn
    /// from, as the Herdr 1 space row sends today (`sidebar::view`).
    pub(crate) fn tap(&self) -> SidebarTap {
        SidebarTap::Space {
            host: self.host.clone(),
            id: self.entry.id.clone(),
            boot: self.boot.clone(),
            generation: self.generation,
        }
    }
}

/// One row of the merged list, in drawing order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ListRow {
    Host(ListHost),
    Space(ListSpace),
    Forward(ListForward),
}

/// A port forward of a host (issue 140): drawn under its host row, before
/// the host's spaces, whatever the host's connection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ListForward {
    /// The host's destination.
    pub(crate) host: String,
    pub(crate) remote: u16,
    pub(crate) state: crate::forwards::State,
}

/// `rows` with each host's forwards right after its host row, in the
/// order they were started. Forwards live beside the readers, outliving a
/// connection and existing for an offline host, so they are not part of
/// [`merge`]'s input.
pub(crate) fn with_forwards(
    rows: Vec<ListRow>,
    forwards: &crate::forwards::Forwards,
) -> Vec<ListRow> {
    let mut out = Vec::with_capacity(rows.len() + forwards.entries().len());
    for row in rows {
        let host = match &row {
            ListRow::Host(host) => Some(host.name.destination().to_owned()),
            _ => None,
        };
        out.push(row);
        if let Some(host) = host {
            out.extend(forwards.for_host(&host).map(|forward| {
                ListRow::Forward(ListForward {
                    host: host.clone(),
                    remote: forward.remote.get(),
                    state: forward.state,
                })
            }));
        }
    }
    out
}

/// The one input of [`merge`]: Herdr 1 connections from
/// `sidebar::view::host_entries` (stream A), Herdr 2 connections from
/// `herdr2_frame::sidebar::herdr2_hosts` (stream F) and from nowhere else.
///
/// `host_entries` lists every reader the app holds, Herdr 2 ones too, but
/// without their sessions; `herdr2_hosts` lists the same Herdr 2 readers with
/// them. Concatenating the two would hand `merge` two Herdr 2 entries for one
/// destination, and `merge` keeps the first of a kind, so the sessions would
/// vanish. Here a Herdr 2 entry of `host_entries` is replaced, where it
/// stands, by the `herdr2_hosts` entry of the same destination. A Herdr 2
/// reader with no session list yet keeps its sessionless entry, so its host
/// row and state still show. A `herdr2_hosts` entry with no reader in
/// `host_entries` follows the rest in its own order.
pub(crate) fn sources(
    host_entries: Vec<HostEntry>,
    herdr2_hosts: Vec<HostEntry>,
) -> Vec<HostEntry> {
    let mut herdr2 = herdr2_hosts
        .into_iter()
        .filter(|entry| entry.kind == HerdrKind::Herdr2)
        .map(Some)
        .collect::<Vec<_>>();
    let mut entries = host_entries
        .into_iter()
        .map(|entry| {
            if entry.kind != HerdrKind::Herdr2 {
                return entry;
            }
            herdr2
                .iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|h2| h2.host == entry.host))
                .and_then(Option::take)
                .unwrap_or(entry)
        })
        .collect::<Vec<_>>();
    entries.extend(herdr2.into_iter().flatten());
    entries
}

/// Folds the per-connection entries, given in Hosts order
/// (`model::order_hosts`) and built by [`sources`], into the list. A host's
/// row stands where its first entry stands; its Herdr 1 spaces follow, then
/// its Herdr 2 sessions. A connection that is not connected adds its state
/// to the host row and no spaces (`HostEntry::with_content` keeps none then).
pub(crate) fn merge(hosts: &[HostEntry], setup: &Setup) -> Vec<ListRow> {
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
        let name = setup.host_name(host);
        let spaces = entries
            .iter()
            .flat_map(|entry| {
                entry.spaces.iter().map(|space| {
                    ListRow::Space(ListSpace {
                        host: name.destination().to_owned(),
                        kind: entry.kind,
                        boot: entry.boot.clone(),
                        generation: entry.generation,
                        entry: space.clone(),
                    })
                })
            })
            .collect::<Vec<_>>();
        rows.push(ListRow::Host(ListHost {
            name,
            connections: entries
                .iter()
                .map(|entry| HostConnection {
                    kind: entry.kind,
                    link: entry.link,
                })
                .collect(),
            selected: entries.iter().any(|entry| entry.selected),
        }));
        rows.extend(spaces);
    }
    rows
}

#[cfg(test)]
#[path = "sidebar_list_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sidebar_list_reconnect_tests.rs"]
mod reconnect_tests;
