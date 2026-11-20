//! L8: Herdr 2 in the shared frame (plan v2 7.14). Owner: L8.
//!
//! Herdr 2 draws in the same frame as Herdr 1; on screen the only difference
//! is "Herdr 2" in the status bar. The one session (`default`,
//! `core/store.rs`) is one space row with one tab. AGENTS lists only the
//! terminals that reported a status (Q4), mapped from F1's raw `CoreStatus`
//! to the sidebar's seven states, so Error sorts right after Needs you (Q5).
//! Connections are keyed by (host, kind), so one host can hold Herdr 1 and
//! Herdr 2 side by side, Herdr 1 first. `session` builds the rows.
use super::{
    HerdrKind,
    sidebar::{AgentIdentity, target::notification_identity},
    sidebar_status::SidebarStatus,
};
use crate::{app::Smoke, notifications::Context, terminal_view::TerminalView};
use sheprd_core_client::wire::StatusState;

pub(crate) mod panes;
pub(crate) mod session;
pub(crate) mod sidebar;

/// The one Herdr 2 session yebisu attaches to (`core::connect`,
/// `core/store.rs`).
pub(crate) const HERDR2_SESSION: &str = "default";
/// q4: New space creates Herdr 1 spaces only until the Herdr 2 sessions task.
pub(crate) const HERDR2_NEW_SPACE: &str = "Herdr 2 has one session per host for now";

#[derive(Debug, Default)]
pub(crate) struct Herdr2State {}

/// One connection per (host, kind).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ConnectionKey {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
}

impl ConnectionKey {
    pub(crate) fn new(host: &str, kind: HerdrKind) -> Self {
        Self {
            host: host.to_owned(),
            kind,
        }
    }

    /// The connection a terminal view holds: its target and its daemon.
    pub(crate) fn of(view: &TerminalView) -> Self {
        let kind = if view.core_mode {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        };
        Self::new(&view.reconnect.target, kind)
    }
}

/// F1: a terminal's address and its status as herdr-core reported it
/// (herdr-gpui 1e105efaf, in the pinned 9327f448). `CoreStatus::from(&Terminal)`
/// reads a listed terminal; `CoreBackend::statuses` lists them all.
pub(crate) use sheprd_core_client::CoreStatus;

/// F1's raw status to the sidebar's seven states. A terminal that reported
/// nothing (or is not listed) is NoStatus, never Unknown (F1
/// `raw_status_none_when_unreported`); Error stays Error, never Needs you.
pub(crate) fn core_status_to_sidebar(status: Option<&CoreStatus>) -> SidebarStatus {
    match status.and_then(|s| s.status.as_ref()).map(|s| s.state) {
        None => SidebarStatus::NoStatus,
        Some(StatusState::Blocked) => SidebarStatus::NeedsYou,
        Some(StatusState::Error) => SidebarStatus::Error,
        Some(StatusState::Working) => SidebarStatus::Working,
        Some(StatusState::Done) => SidebarStatus::Done,
        Some(StatusState::Idle) => SidebarStatus::Idle,
        Some(StatusState::Other) => SidebarStatus::Unknown,
    }
}

/// What New space offers for one Herdr kind on a host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NewSpaceChoice {
    pub(crate) kind: HerdrKind,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

/// New space for the kinds a host runs, Herdr 1 first. Only Herdr 1 creates
/// a space; Herdr 2 is shown disabled with its reason until the Herdr 2
/// sessions task (q4; the 2026-11-11 default is Herdr 1).
pub(crate) fn new_space_choices(kinds: &[HerdrKind]) -> Vec<NewSpaceChoice> {
    let mut kinds = kinds.to_vec();
    kinds.sort();
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| match kind {
            HerdrKind::Herdr1 => NewSpaceChoice {
                kind,
                enabled: true,
                reason: None,
            },
            HerdrKind::Herdr2 => NewSpaceChoice {
                kind,
                enabled: false,
                reason: Some(HERDR2_NEW_SPACE),
            },
        })
        .collect()
}

/// The agent a notification is about, on the connection that delivered it:
/// the identity keeps that connection's Herdr kind, so a Herdr 2 terminal
/// never resolves to a Herdr 1 pane of the same host (or the reverse).
pub(crate) fn notification_agent(target: &Context, key: &ConnectionKey) -> Option<AgentIdentity> {
    if target.host != key.host {
        return None;
    }
    notification_identity(target, key.kind)
}

impl Smoke {
    /// Whether the Herdr 2 control band (CB) stays in the terminal screen:
    /// until every control it holds (Split, Pane 1, Pane 2, the split
    /// direction, Detach) has a home in the ... menu (plan v2 5.2, L8 column;
    /// 5.1 rule 1).
    pub(crate) fn core_controls_in_band(&self) -> bool {
        !panes::band_retired(&self.chrome_state())
    }
}

#[cfg(test)]
#[path = "herdr2_frame_tests.rs"]
mod tests;
