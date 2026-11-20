//! Herdr 2 session and command routing in the shared frame.
use std::collections::BTreeMap;

use super::{
    HerdrKind,
    sidebar::{AgentIdentity, target::notification_identity},
    sidebar_status::SidebarStatus,
};
use crate::{app::Smoke, notifications::Context, terminal_view::TerminalView};
use sheprd_core_client::wire::StatusState;

mod new_space;
pub(crate) mod panes;
pub(crate) mod session;
pub(crate) mod sidebar;

/// The session used by installs that have not picked a named space.
pub(crate) const HERDR2_SESSION: &str = "default";
#[derive(Debug, Default)]
pub(crate) struct Herdr2State {
    selected: BTreeMap<String, String>,
    new_space: Option<new_space::Chooser>,
    #[cfg(all(target_os = "ios", not(test)))]
    host_probe: Option<gpui::Task<()>>,
    #[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
    wave1_qa_step: u8,
}

impl Herdr2State {
    pub(crate) fn selected_session(&self, host: &str) -> &str {
        self.selected
            .get(host)
            .map_or(HERDR2_SESSION, String::as_str)
    }

    pub(crate) fn confirm_session(&mut self, host: &str, session: &str) {
        if self
            .selected
            .get(host)
            .is_none_or(|selected| selected != session)
        {
            self.selected.insert(host.to_owned(), session.to_owned());
        }
    }
}

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

/// Both installed kinds are offered; Herdr 2 is initially selected.
pub(crate) fn new_space_choices(kinds: &[HerdrKind]) -> Vec<HerdrKind> {
    let mut kinds = kinds.to_vec();
    kinds.sort();
    kinds.dedup();
    kinds
}

/// The agent a notification is about, on the connection that delivered it:
/// the identity keeps that connection's Herdr kind, so a Herdr 2 terminal
/// never resolves to a Herdr 1 pane of the same host (or the reverse).
pub(crate) fn notification_agent(target: &Context, key: &ConnectionKey) -> Option<AgentIdentity> {
    if target.host != key.host || target.herdr != key.kind {
        return None;
    }
    notification_identity(target)
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
