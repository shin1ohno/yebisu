//! L3: host, space and AGENTS rows built from daemon snapshots. Pure data;
//! the view draws it. Owner: L3.
use super::{
    AgentIdentity, AgentRow, Observed, SidebarState, agent_order, agent_order_in, display_name,
};
use crate::{
    board::snapshot,
    hosts::HostName,
    live::Live,
    shell::{
        HerdrKind,
        sidebar_status::{SidebarStatus, from_agent_status},
    },
};
use sheprd_client::{
    herdr::AgentKind,
    protocol::{ClientShellAgent, ClientShellSnapshot},
    text::{caption, safe_text},
};

// R26: the fork's docs/agent-labels.md limits pane and task captions to 160 characters.
const CAPTION_LIMIT: usize = 160;

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
            state.pins.borrow_mut().reconcile(host, snap);
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
    pub(super) fn tab_count(&self) -> String {
        let unit = if self.tabs == 1 { "tab" } else { "tabs" };
        format!("{} {unit}", self.tabs)
    }

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
    space_entry_with_reason(kind, input, rows, |_| None)
}

fn space_entry_with_reason(
    kind: HerdrKind,
    input: SpaceInput,
    rows: &[AgentRow],
    wait_reason: impl FnOnce(&AgentRow) -> Option<String>,
) -> SpaceEntry {
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
            let status = (lead.status == SidebarStatus::NeedsYou)
                .then(|| wait_reason(lead))
                .flatten();
            let status = status.as_deref().unwrap_or_else(|| lead.status.label());
            let mut detail = format!("{} · {status}", display_name(lead, &agents));
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

    pub(super) fn display_name(&self) -> String {
        HostName::new(&self.host).display().to_owned()
    }

    /// The existing Herdr 2 renderer's detail, until A2 switches it over.
    pub(crate) fn detail(&self) -> &'static str {
        match (self.link, self.kind) {
            (HostLink::Connected, HerdrKind::Herdr1) => "SSH · Herdr 1",
            (HostLink::Connected, HerdrKind::Herdr2) => "SSH · Herdr 2",
            (HostLink::Connecting, _) => "Connecting…",
            (HostLink::Offline(reason), _) => reason.label(),
        }
    }

    /// The connection type and state are available as text as well as a dot.
    pub(super) fn connection_detail(&self) -> String {
        let version = match self.kind {
            HerdrKind::Herdr1 => "Herdr 1",
            HerdrKind::Herdr2 => "Herdr 2",
        };
        let status = match self.link {
            HostLink::Connected => "Connected",
            HostLink::Connecting => "Connecting…",
            HostLink::Offline(reason) => reason.label(),
        };
        format!("SSH · {version} · {status}")
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
    agents_list_with_pins(hosts, &|_, _| None)
}

impl SidebarState {
    /// The same AGENTS order for surfaces outside the sidebar.
    pub(crate) fn ordered_agents(&self, hosts: &[HostEntry]) -> Vec<AgentLine> {
        agents_list_with_pins(hosts, &|id, boot| self.pins.borrow().index(id, boot))
    }
}

pub(super) fn agents_list_with_pins(
    hosts: &[HostEntry],
    pinned: &dyn Fn(&AgentIdentity, &str) -> Option<usize>,
) -> Vec<AgentLine> {
    let mut lines = hosts
        .iter()
        .flat_map(|host| {
            host.agents
                .iter()
                .filter(|line| line.row.status.is_agent())
                .map(move |line| (pinned(&line.row.identity, &host.boot), line))
        })
        .collect::<Vec<_>>();
    let host_rank = |id: &AgentIdentity| {
        hosts
            .iter()
            .position(|host| host.host == id.host && host.kind == id.kind)
            .unwrap_or(usize::MAX)
    };
    lines.sort_by(|(a_pin, a), (b_pin, b)| {
        agent_order_in(&a.row, &b.row, &host_rank, *a_pin, *b_pin)
    });
    lines.into_iter().map(|(_, line)| line.clone()).collect()
}

fn safe_caption(value: &str) -> Option<String> {
    let label = safe_text(caption(value)?, CAPTION_LIMIT);
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_owned())
}

fn kind_only(label: &str, agent: &ClientShellAgent) -> bool {
    let identity = label.to_ascii_lowercase();
    // AgentIcon in the fork also covers kinds with no session-resume adapter.
    AgentKind::parse(&identity).is_some()
        || matches!(
            identity.as_str(),
            "agent" | "claude code" | "omo" | "gemini" | "cline" | "kiro" | "amp" | "maki" | "muse"
        )
        || agent
            .agent
            .as_deref()
            .is_some_and(|kind| label.eq_ignore_ascii_case(kind.trim()))
}

fn agent_name(agent: &ClientShellAgent, snapshot: &ClientShellSnapshot) -> String {
    let pane = snapshot.panes.iter().find(|pane| {
        pane.pane_id == agent.pane_id
            && pane.tab_id == agent.tab_id
            && pane.workspace_id == agent.workspace_id
    });
    if let Some(rename) = pane
        .and_then(|pane| pane.label.as_deref())
        .and_then(safe_caption)
    {
        return rename;
    }
    let name = [&agent.title, &agent.name, &agent.display_agent]
        .into_iter()
        .filter_map(|value| value.as_deref().and_then(safe_caption))
        .find(|label| !kind_only(label, agent));
    name.unwrap_or_else(|| {
        let tab = snapshot
            .tabs
            .iter()
            .find(|tab| tab.tab_id == agent.tab_id && tab.workspace_id == agent.workspace_id)
            .and_then(|tab| safe_caption(&tab.label));
        let id = safe_text(&agent.pane_id, CAPTION_LIMIT);
        tab.map_or_else(
            || id.clone(),
            |tab| safe_text(&format!("{id} · {tab}"), CAPTION_LIMIT),
        )
    })
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
    safe_caption(label).unwrap_or_else(|| format!("Space {number}"))
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
            name: agent_name(agent, snapshot),
            tab: tab_number(&agent.tab_id),
            status: from_agent_status(agent.agent_status),
        }
    };
    let display_host = HostName::new(host);
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
                place: format!("{name} · {display_host}"),
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
        spaces.push(space_entry_with_reason(kind, input, &rows, |lead| {
            snapshot
                .agents
                .iter()
                .find(|agent| {
                    agent.pane_id == lead.identity.pane && agent.workspace_id == space.workspace_id
                })?
                .state_labels
                .iter()
                .filter(|(state, _)| state == "blocked")
                .find_map(|(_, reason)| safe_caption(reason))
        }));
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
            place: display_host.to_string(),
            row,
        });
    }
    (spaces, lines)
}
