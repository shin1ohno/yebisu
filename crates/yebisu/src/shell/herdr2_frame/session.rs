//! L8: the Herdr 2 session as sidebar rows (plan v2 7.14). Owner: L8.
//!
//! One session is one space row with one tab (q4). AGENTS rows are the
//! terminals with a reported status (Q4), keyed (host, Herdr 2, address)
//! (D8), in L3's Q5 order. Terminal text (titles, app names) is untrusted:
//! control characters become spaces so a row stays one line.
use super::{CoreStatus, HERDR2_SESSION, core_status_to_sidebar};
use crate::shell::{
    HerdrKind,
    sidebar::{
        AgentIdentity, AgentRow, Observed, SidebarState,
        model::{AgentLine, HostEntry, HostLink, SpaceEntry, SpaceInput, space_entry},
    },
};

/// The tab of the one session: (tab id, label, focused), the tab row's
/// shape (the row numbers it "1 · default"). Herdr 2 has no tab ids; the
/// session name stands in.
pub(crate) fn session_tabs() -> Vec<(String, String, bool)> {
    vec![(HERDR2_SESSION.to_owned(), HERDR2_SESSION.to_owned(), true)]
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Row name: the program's title, then its app, then the address.
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

/// The receive-order key of one Herdr 2 connection: a new generation is a
/// new connection, so its first snapshot is a first batch (L3 ordering).
fn batch_key(generation: u64) -> String {
    format!("herdr2-{generation}")
}

/// The session's space row and its AGENTS lines, recording the receive
/// order in `state` first. Status-less terminals are left out of AGENTS but
/// still belong to the session (its row says No agent when none reported).
pub(crate) fn session_content(
    host: &str,
    generation: u64,
    statuses: &[CoreStatus],
    state: &SidebarState,
) -> (Vec<SpaceEntry>, Vec<AgentLine>) {
    let reported = statuses
        .iter()
        .map(|s| (s, core_status_to_sidebar(Some(s))))
        .filter(|(_, status)| status.is_agent())
        .collect::<Vec<_>>();
    let observed = reported
        .iter()
        .map(|(s, status)| Observed {
            pane: s.address.clone(),
            status: *status,
            seq: 0,
        })
        .collect::<Vec<_>>();
    state.observe(host, HerdrKind::Herdr2, &batch_key(generation), &observed);
    let rows = reported
        .iter()
        .map(|(s, status)| {
            let identity = AgentIdentity {
                host: host.to_owned(),
                kind: HerdrKind::Herdr2,
                pane: s.address.clone(),
            };
            AgentRow {
                received: state.received(&identity),
                identity,
                name: agent_name(s),
                tab: 1,
                status: *status,
            }
        })
        .collect::<Vec<_>>();
    let space = space_entry(
        HerdrKind::Herdr2,
        SpaceInput {
            id: HERDR2_SESSION.to_owned(),
            name: HERDR2_SESSION.to_owned(),
            tabs: 1,
            branch: None,
            current: true,
        },
        &rows,
    );
    let lines = rows
        .iter()
        .map(|row| AgentLine {
            shown: crate::shell::sidebar::display_name(row, &rows),
            place: format!("{HERDR2_SESSION} · {host}"),
            row: row.clone(),
        })
        .collect();
    (vec![space], lines)
}

/// A Herdr 2 host row with its session while connected. A host that is not
/// connected keeps no session row and forgets its receive order.
pub(crate) fn host_entry(
    host: &str,
    link: HostLink,
    generation: u64,
    statuses: &[CoreStatus],
    state: &SidebarState,
) -> HostEntry {
    let mut entry = HostEntry::new(host, HerdrKind::Herdr2, link);
    entry.generation = generation;
    if link != HostLink::Connected {
        state.forget(host, HerdrKind::Herdr2);
        return entry;
    }
    let (spaces, agents) = session_content(host, generation, statuses, state);
    entry.with_content(spaces, agents)
}

/// Whether the sidebar has a list to draw. Herdr 1 always does. Herdr 2 has
/// one only once L8 supplies a Herdr 2 host row; with no host the list is
/// `None`, so the frame draws no column and gives the 300 pt back to the
/// terminal (5ff25e7, Integration 1 review S3).
pub(crate) fn sidebar_list_ready(core_mode: bool, hosts: &[HostEntry]) -> bool {
    !core_mode || hosts.iter().any(|h| h.kind == HerdrKind::Herdr2)
}
