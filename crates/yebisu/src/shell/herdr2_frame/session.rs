//! Translate the worker's session catalog and terminal statuses to sidebar rows.
use super::{CoreStatus, HERDR2_SESSION, core_status_to_sidebar};
use crate::{
    core::{DocRows, SessionInfo},
    shell::{
        HerdrKind,
        sidebar::{
            AgentIdentity, AgentRow, Observed, SidebarState,
            agent_icon::AgentIcon,
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

/// A document space row is `default\0<space id>`, NUL-namespaced like the
/// agent rows (#144 D2); session names cannot hold NUL (`parse_discovery`).
fn space_identity(space: &str) -> String {
    format!("{HERDR2_SESSION}\0{space}")
}

/// A space row's session, and its document space when it is one.
pub(super) fn split_space_identity(identity: &str) -> (&str, Option<&str>) {
    match identity.split_once('\0') {
        Some((session, space)) => (session, Some(space)),
        None => (identity, None),
    }
}

/// The detail of a row this core's official app never shows (decision 3).
const IPAD_ONLY: &str = "iPad only";
/// The `default` row of a document the official app could not read (Q1).
const UNREADABLE: &str = "Can't read the herdr-app layout";

/// The document space and tab position of `address` in `session`.
fn doc_place<'a>(session: &'a SessionInfo, address: &str) -> Option<(&'a str, &'a str, usize)> {
    let Some(DocRows::Readable(spaces)) = session.doc.as_ref().map(|doc| &doc.state) else {
        return None;
    };
    spaces.iter().find_map(|space| {
        let tab = space
            .tabs
            .iter()
            .position(|tab| tab.panes.iter().any(|pane| pane == address))?;
        Some((space.id.as_str(), space.name.as_str(), tab))
    })
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
            let tab = doc_place(session, &entry.address)
                .map(|(_, _, tab)| tab)
                .or_else(|| {
                    session
                        .tabs
                        .iter()
                        .position(|tab| tab.panes.contains(&entry.address))
                })
                .map_or(1, |index| index + 1);
            AgentRow {
                received: state.received(&identity),
                identity,
                name: agent_name(entry),
                tab: tab as u64,
                status,
                icon: AgentIcon::from_app(entry.status.as_ref().map(|s| s.app.as_str())),
            }
        })
        .collect::<Vec<_>>();
    let documented = catalog
        .iter()
        .find(|entry| entry.name == HERDR2_SESSION && entry.doc.is_some());
    let spaces = catalog
        .iter()
        .flat_map(|entry| {
            let agents = rows
                .iter()
                .filter(|row| split_pane_identity(&row.identity.pane).0 == entry.name)
                .cloned()
                .collect::<Vec<_>>();
            if let Some(doc) = &entry.doc {
                return doc_rows(entry, doc, &agents, entry.name == current);
            }
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
            } else if documented.is_some() {
                space.detail = IPAD_ONLY.into();
            }
            vec![space]
        })
        .collect();
    let lines = rows
        .iter()
        .map(|row| {
            let (session, address) = split_pane_identity(&row.identity.pane);
            let place = catalog
                .iter()
                .find(|entry| entry.name == session)
                .and_then(|entry| doc_place(entry, address));
            AgentLine {
                shown: crate::shell::sidebar::display_name(row, &rows),
                space: Some(
                    place.map_or_else(|| one_line(session), |(_, name, _)| name.to_owned()),
                ),
                row: row.clone(),
            }
        })
        .collect();
    (spaces, lines)
}

/// The `default` session's rows when it read the document (Design F): one
/// per document space in its order, then the iPad-only tabs as `default`
/// while there are any. An unreadable document is one `default` row.
fn doc_rows(
    entry: &SessionInfo,
    doc: &crate::core::DocCatalog,
    agents: &[AgentRow],
    current: bool,
) -> Vec<SpaceEntry> {
    let local = |tabs: usize, agents: &[AgentRow], detail: &str| {
        let mut space = space_entry(
            HerdrKind::Herdr2,
            SpaceInput {
                id: entry.name.clone(),
                name: one_line(&entry.name),
                tabs,
                branch: None,
                current: current && doc.selected.is_none(),
            },
            agents,
        );
        space.detail = detail.into();
        space
    };
    let DocRows::Readable(spaces) = &doc.state else {
        return vec![local(entry.tabs.len(), agents, UNREADABLE)];
    };
    let place = |row: &AgentRow| doc_place(entry, split_pane_identity(&row.identity.pane).1);
    let mut rows = spaces
        .iter()
        .map(|space| {
            let agents = agents
                .iter()
                .filter(|row| place(row).is_some_and(|(id, _, _)| id == space.id))
                .cloned()
                .collect::<Vec<_>>();
            space_entry(
                HerdrKind::Herdr2,
                SpaceInput {
                    id: space_identity(&space.id),
                    name: space.name.clone(),
                    tabs: space.tabs.len(),
                    branch: None,
                    current: current && doc.selected.as_deref() == Some(space.id.as_str()),
                },
                &agents,
            )
        })
        .collect::<Vec<_>>();
    if doc.local_tabs > 0 {
        let agents = agents
            .iter()
            .filter(|row| place(row).is_none())
            .cloned()
            .collect::<Vec<_>>();
        rows.push(local(doc.local_tabs, &agents, IPAD_ONLY));
    }
    rows
}

#[cfg(test)]
pub(super) fn bootstrap_catalog(name: &str) -> Vec<SessionInfo> {
    vec![SessionInfo {
        name: name.to_owned(),
        confirmed: true,
        core_id: None,
        created: 0,
        statuses: Vec::new(),
        doc: None,
        tabs: vec![crate::core::TabInfo {
            id: name.to_owned(),
            label: name.to_owned(),
            active: true,
            panes: Vec::new(),
            focused_pane: 0,
            vertical: false,
            auto_name: false,
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
