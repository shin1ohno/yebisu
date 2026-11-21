//! Translate the worker's session catalog and terminal statuses to sidebar rows.
use super::{CoreStatus, HERDR2_SESSION, core_status_to_sidebar};
use crate::{
    core::SessionInfo,
    hosts::HostName,
    shell::{
        HerdrKind,
        sidebar::{
            AgentIdentity, AgentRow, Observed, SidebarState,
            model::{AgentLine, HostEntry, HostLink, SpaceEntry, SpaceInput, space_entry},
        },
    },
};

/// The legacy shared tab strip continues to use this until C2 connects
/// `Smoke::core_tabs` and the shared command entry points.
pub(crate) fn session_tabs() -> Vec<(String, String, bool)> {
    vec![(HERDR2_SESSION.to_owned(), HERDR2_SESSION.to_owned(), true)]
}

pub(crate) fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(160)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn agent_name(status: &CoreStatus) -> String {
    status
        .status
        .as_ref()
        .into_iter()
        .flat_map(|s| [s.title.as_str(), s.app.as_str()])
        .map(one_line)
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| one_line(&status.address))
}

/// A pane address is local to a core daemon, so named sessions namespace
/// their pane identities. Preserve identities already stored for default.
fn pane_identity(session: &str, address: &str) -> String {
    if session == HERDR2_SESSION {
        address.to_owned()
    } else {
        format!("{session}\0{address}")
    }
}

pub(super) fn split_pane_identity(identity: &str) -> (&str, &str) {
    identity
        .split_once('\0')
        .unwrap_or((HERDR2_SESSION, identity))
}

fn batch_key(generation: u64) -> String {
    format!("herdr2-{generation}")
}

/// One observation batch per host includes every confirmed session.
pub(crate) fn catalog_content(
    host: &str,
    generation: u64,
    current: &str,
    catalog: &[SessionInfo],
    statuses: &[CoreStatus],
    state: &SidebarState,
) -> (Vec<SpaceEntry>, Vec<AgentLine>) {
    let reported = catalog
        .iter()
        .filter(|session| session.confirmed)
        .flat_map(|session| {
            let statuses = if session.name == current {
                statuses
            } else {
                &session.statuses
            };
            statuses
                .iter()
                .map(move |entry| ((session, entry), core_status_to_sidebar(Some(entry))))
        })
        .filter(|(_, status)| status.is_agent())
        .collect::<Vec<_>>();
    let observed = reported
        .iter()
        .map(|((session, entry), status)| Observed {
            pane: pane_identity(&session.name, &entry.address),
            status: *status,
            seq: 0,
        })
        .collect::<Vec<_>>();
    state.observe(host, HerdrKind::Herdr2, &batch_key(generation), &observed);

    let rows = reported
        .into_iter()
        .map(|((session, entry), status)| {
            let identity = AgentIdentity {
                host: host.to_owned(),
                kind: HerdrKind::Herdr2,
                pane: pane_identity(&session.name, &entry.address),
            };
            let tab = session
                .tabs
                .iter()
                .position(|tab| tab.panes.contains(&entry.address))
                .map_or(1, |index| index + 1);
            AgentRow {
                received: state.received(&identity),
                identity,
                name: agent_name(entry),
                tab: tab as u64,
                status,
            }
        })
        .collect::<Vec<_>>();
    let spaces = catalog
        .iter()
        .map(|entry| {
            let agents = rows
                .iter()
                .filter(|row| split_pane_identity(&row.identity.pane).0 == entry.name)
                .cloned()
                .collect::<Vec<_>>();
            let mut space = space_entry(
                HerdrKind::Herdr2,
                SpaceInput {
                    id: entry.name.clone(),
                    name: one_line(&entry.name),
                    tabs: entry.tabs.len(),
                    branch: None,
                    current: entry.name == current,
                },
                &agents,
            );
            if !entry.confirmed {
                space.detail = "Session unconfirmed".into();
            }
            space
        })
        .collect();
    let lines = rows
        .iter()
        .map(|row| AgentLine {
            shown: crate::shell::sidebar::display_name(row, &rows),
            place: format!(
                "{} · {}",
                one_line(split_pane_identity(&row.identity.pane).0),
                HostName::new(host),
            ),
            row: row.clone(),
        })
        .collect();
    (spaces, lines)
}

#[cfg(test)]
pub(super) fn bootstrap_catalog(name: &str) -> Vec<SessionInfo> {
    vec![SessionInfo {
        name: name.to_owned(),
        confirmed: true,
        core_id: None,
        created: 0,
        statuses: Vec::new(),
        tabs: vec![crate::core::TabInfo {
            id: name.to_owned(),
            label: name.to_owned(),
            active: true,
            panes: Vec::new(),
            focused_pane: 0,
            vertical: false,
        }],
    }]
}

/// A Herdr 2 host keeps its identity when disconnected and forgets its
/// session rows and receive order until a new connection publishes them.
pub(crate) fn host_entry(
    mut entry: HostEntry,
    current: &str,
    catalog: &[SessionInfo],
    statuses: &[CoreStatus],
    state: &SidebarState,
) -> HostEntry {
    if entry.link != HostLink::Connected {
        state.forget(&entry.host, HerdrKind::Herdr2);
        return entry;
    }
    let (spaces, agents) = catalog_content(
        &entry.host,
        entry.generation,
        current,
        catalog,
        statuses,
        state,
    );
    entry.spaces = spaces;
    entry.agents = agents;
    entry
}
