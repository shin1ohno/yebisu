//! L3: host, space and AGENTS rows built from daemon snapshots. Pure data;
//! the view draws it. Owner: L3.
use super::{
    AgentIdentity, AgentRow, Observed, SidebarState, agent_order, agent_order_in, display_name,
};
use crate::{
    board::snapshot,
    live::Live,
    shell::{
        HerdrKind,
        sidebar_status::{SidebarStatus, from_agent_status},
    },
};
use sheprd_client::protocol::{ClientShellAgent, ClientShellSnapshot};

/// Why a host is not connected (SidebarSpec 4). Fixed labels only: the
/// daemon's or SSH's own text is untrusted and is never drawn here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OfflineReason {
    NotConnected,
    HostKeyMismatch,
    SignInFailed,
    HerdrNotRunning,
    VersionMismatch,
}

impl OfflineReason {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NotConnected => "Not connected",
            Self::HostKeyMismatch => "Host key mismatch",
            Self::SignInFailed => "Sign-in failed",
            Self::HerdrNotRunning => "Herdr not running",
            Self::VersionMismatch => "Version mismatch",
        }
    }
}

/// A host row's connection state (the host dot and its second line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostLink {
    Connected,
    Connecting,
    Offline(OfflineReason),
}

/// Maps a stop text to a fixed reason. The text only selects a label.
pub(crate) fn offline_reason(text: &str) -> OfflineReason {
    let text = text.to_ascii_lowercase();
    if text.contains("version mismatch") {
        OfflineReason::VersionMismatch
    } else if text.contains("host key") || text.contains("untrusted") {
        OfflineReason::HostKeyMismatch
    } else if text.contains("authentication")
        || text.contains("permission denied")
        || text.contains("sign-in")
        || text.contains("sign in")
    {
        OfflineReason::SignInFailed
    } else if text.contains("not running")
        || text.contains("connection refused")
        || text.contains("no such file")
    {
        OfflineReason::HerdrNotRunning
    } else {
        OfflineReason::NotConnected
    }
}

/// Records a reader update in the receive order when it arrives (called
/// from `board.rs` `record_board` for every host, drawn or not). A host that
/// is not connected forgets its agents, so its next snapshot is a first batch.
pub(crate) fn record_live(state: &SidebarState, host: &str, kind: HerdrKind, live: &Live) {
    if host.is_empty() {
        return;
    }
    match (host_link(live), kind, snapshot(live)) {
        (HostLink::Connected, HerdrKind::Herdr1, Some(snap)) => {
            state.observe(host, kind, &snap.boot_id, &observed(snap));
        }
        (HostLink::Connected, _, _) => {}
        _ => state.forget(host, kind),
    }
}

/// The connection state of one host's reader.
pub(crate) fn host_link(live: &Live) -> HostLink {
    if live.version_mismatch.get().is_some() {
        return HostLink::Offline(OfflineReason::VersionMismatch);
    }
    if live.connected {
        return HostLink::Connected;
    }
    if live.status.starts_with("Connecting") {
        return HostLink::Connecting;
    }
    HostLink::Offline(offline_reason(&live.status))
}

/// What a space row needs from its daemon (Herdr 1 workspace or Herdr 2
/// session).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpaceInput {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tabs: usize,
    pub(crate) branch: Option<String>,
    pub(crate) current: bool,
}

/// One space row (56 pt): dot, name, tab count, Needs you and a single
/// Error tag; the second line is the lead agent and status, or No agent.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpaceEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tabs: usize,
    pub(crate) dot: SidebarStatus,
    pub(crate) needs_you: bool,
    pub(crate) error_tag: bool,
    pub(crate) detail: String,
    pub(crate) current: bool,
}

impl SpaceEntry {
    /// Tags after the name: at most one Needs you and one Error (q3).
    pub(crate) fn tags(&self) -> Vec<&'static str> {
        let mut tags = Vec::new();
        if self.needs_you {
            tags.push(SidebarStatus::NeedsYou.label());
        }
        if self.error_tag {
            tags.push(SidebarStatus::Error.label());
        }
        tags
    }
}

/// Builds a space row from its terminals. Only status-sending terminals are
/// agents (Q4); the branch shows only for Herdr 1 and never for `~` (D3).
pub(crate) fn space_entry(kind: HerdrKind, input: SpaceInput, rows: &[AgentRow]) -> SpaceEntry {
    let mut agents = rows
        .iter()
        .filter(|row| row.status.is_agent())
        .cloned()
        .collect::<Vec<_>>();
    agents.sort_by(agent_order);
    let branch = match kind {
        HerdrKind::Herdr1 => input
            .branch
            .filter(|b| !b.trim().is_empty() && !b.starts_with('~')),
        HerdrKind::Herdr2 => None,
    };
    let (dot, detail) = match agents.first() {
        None => (SidebarStatus::NoStatus, "No agent".to_owned()),
        Some(lead) => {
            let mut detail = format!("{} · {}", display_name(lead, &agents), lead.status.label());
            if let Some(branch) = &branch {
                detail.push_str(" · ");
                detail.push_str(branch);
            }
            (lead.status, detail)
        }
    };
    SpaceEntry {
        id: input.id,
        name: input.name,
        tabs: input.tabs,
        dot,
        needs_you: agents.iter().any(|a| a.status == SidebarStatus::NeedsYou),
        error_tag: agents.iter().any(|a| a.status == SidebarStatus::Error),
        detail,
        current: input.current,
    }
}

/// One AGENTS line (44 pt): the row, its shown name, and its place.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgentLine {
    pub(crate) row: AgentRow,
    pub(crate) shown: String,
    /// Second line: space · host.
    pub(crate) place: String,
}

/// One host row (64 pt) with its spaces and agents.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HostEntry {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) link: HostLink,
    pub(crate) selected: bool,
    pub(crate) boot: String,
    pub(crate) generation: u64,
    pub(crate) spaces: Vec<SpaceEntry>,
    pub(crate) agents: Vec<AgentLine>,
}

impl HostEntry {
    /// A host with no spaces. A host that is not connected keeps none of its
    /// previous spaces or agents (decided 2026-11-11).
    pub(crate) fn new(host: &str, kind: HerdrKind, link: HostLink) -> Self {
        Self {
            host: host.to_owned(),
            kind,
            link,
            selected: false,
            boot: String::new(),
            generation: 0,
            spaces: Vec::new(),
            agents: Vec::new(),
        }
    }

    /// Adds spaces and agents, only while connected.
    pub(crate) fn with_content(mut self, spaces: Vec<SpaceEntry>, agents: Vec<AgentLine>) -> Self {
        if self.link == HostLink::Connected {
            self.spaces = spaces;
            self.agents = agents;
        }
        self
    }

    /// Second line: SSH and the answering Herdr, or why it is not connected.
    pub(crate) fn detail(&self) -> &'static str {
        match (self.link, self.kind) {
            (HostLink::Connected, HerdrKind::Herdr1) => "SSH · Herdr 1",
            (HostLink::Connected, HerdrKind::Herdr2) => "SSH · Herdr 2",
            (HostLink::Connecting, _) => "Connecting…",
            (HostLink::Offline(reason), _) => reason.label(),
        }
    }
}

/// Hosts in the saved Hosts order; hosts missing from it follow by name.
pub(crate) fn order_hosts(hosts: &mut [HostEntry], saved: &[String]) {
    hosts.sort_by(|a, b| {
        let at = saved
            .iter()
            .position(|h| h == &a.host)
            .unwrap_or(usize::MAX);
        let bt = saved
            .iter()
            .position(|h| h == &b.host)
            .unwrap_or(usize::MAX);
        at.cmp(&bt)
            .then_with(|| a.host.cmp(&b.host))
            .then_with(|| a.kind.cmp(&b.kind))
    });
}

/// AGENTS across every host: status-sending terminals only, in Q5 order.
/// `hosts` is in Hosts order (`order_hosts`); a first batch follows it.
pub(crate) fn agents_list(hosts: &[HostEntry]) -> Vec<AgentLine> {
    let host_rank = |id: &AgentIdentity| {
        hosts
            .iter()
            .position(|h| h.host == id.host && h.kind == id.kind)
            .unwrap_or(usize::MAX)
    };
    let mut lines = hosts
        .iter()
        .flat_map(|h| h.agents.iter())
        .filter(|line| line.row.status.is_agent())
        .cloned()
        .collect::<Vec<_>>();
    lines.sort_by(|a, b| agent_order_in(&a.row, &b.row, &host_rank));
    lines
}

fn agent_name(agent: &ClientShellAgent) -> String {
    agent
        .name
        .as_deref()
        .or(agent.title.as_deref())
        .or(agent.display_agent.as_deref())
        .or(agent.agent.as_deref())
        .unwrap_or(&agent.pane_id)
        .to_owned()
}

/// The daemon's agents as this iPad saw them, for `SidebarState::observe`.
pub(crate) fn observed(snapshot: &ClientShellSnapshot) -> Vec<Observed> {
    snapshot
        .agents
        .iter()
        .map(|a| Observed {
            pane: a.pane_id.clone(),
            status: from_agent_status(a.agent_status),
            seq: a.state_change_seq,
        })
        .collect()
}

fn space_name(number: usize, label: &str) -> String {
    format!("Space {number} · {label}")
}

/// Spaces in Herdr 1's order and the host's agents, from one snapshot.
pub(crate) fn snapshot_content(
    host: &str,
    snapshot: &ClientShellSnapshot,
    state: &SidebarState,
) -> (Vec<SpaceEntry>, Vec<AgentLine>) {
    let kind = HerdrKind::Herdr1;
    let tab_number = |tab: &str| {
        snapshot
            .tabs
            .iter()
            .find(|t| t.tab_id == tab)
            .map_or(0, |t| t.number as u64)
    };
    let row_of = |agent: &ClientShellAgent| {
        let identity = AgentIdentity {
            host: host.to_owned(),
            kind,
            pane: agent.pane_id.clone(),
        };
        AgentRow {
            received: state.received(&identity),
            identity,
            name: agent_name(agent),
            tab: tab_number(&agent.tab_id),
            status: from_agent_status(agent.agent_status),
        }
    };
    let mut spaces = Vec::new();
    let mut lines = Vec::new();
    for space in &snapshot.workspaces {
        let name = space_name(space.number, &space.label);
        let rows = snapshot
            .agents
            .iter()
            .filter(|a| a.workspace_id == space.workspace_id)
            .map(row_of)
            .collect::<Vec<_>>();
        for row in &rows {
            lines.push(AgentLine {
                shown: display_name(row, &rows),
                place: format!("{name} · {host}"),
                row: row.clone(),
            });
        }
        let input = SpaceInput {
            id: space.workspace_id.clone(),
            name,
            tabs: snapshot
                .tabs
                .iter()
                .filter(|t| t.workspace_id == space.workspace_id)
                .count(),
            branch: space.branch.clone(),
            current: snapshot.focused_workspace_id.as_ref() == Some(&space.workspace_id),
        };
        spaces.push(space_entry(kind, input, &rows));
    }
    // An agent whose workspace the snapshot omits still reaches AGENTS.
    for agent in snapshot.agents.iter().filter(|a| {
        !snapshot
            .workspaces
            .iter()
            .any(|w| w.workspace_id == a.workspace_id)
    }) {
        let row = row_of(agent);
        lines.push(AgentLine {
            shown: row.name.clone(),
            place: host.to_owned(),
            row,
        });
    }
    (spaces, lines)
}
