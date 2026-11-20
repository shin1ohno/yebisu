//! L3: the sidebar's own status type (S-8) and the AGENTS order with pins
//! (design fidelity plan, Wave 0a). Owner: stream A.
use super::style::color;
use sheprd_client::protocol::AgentStatus;

/// Seven states an AGENTS row or a space dot can show. Herdr 1 supplies five
/// of them; Error and NoStatus come from Herdr 2 (F1 and L8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SidebarStatus {
    NeedsYou,
    Error,
    Working,
    Done,
    Idle,
    Unknown,
    NoStatus,
}

impl SidebarStatus {
    /// Every value, in AGENTS order (Q5).
    pub(crate) const ALL: [Self; 7] = [
        Self::NeedsYou,
        Self::Error,
        Self::Working,
        Self::Done,
        Self::Idle,
        Self::Unknown,
        Self::NoStatus,
    ];

    /// AGENTS order (Q5): Needs you, Error, Working, Done, Idle, Unknown.
    /// NoStatus is never an agent (Q4); it sorts last for space dots only.
    pub(crate) const fn rank(self) -> u8 {
        match self {
            Self::NeedsYou => 0,
            Self::Error => 1,
            Self::Working => 2,
            Self::Done => 3,
            Self::Idle => 4,
            Self::Unknown => 5,
            Self::NoStatus => 6,
        }
    }

    /// A terminal that sends no status is a plain shell, not an agent (Q4).
    pub(crate) const fn is_agent(self) -> bool {
        !matches!(self, Self::NoStatus)
    }

    /// Row text; the colour is never the only signal (SidebarSpec legend).
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs you",
            Self::Error => "Error",
            Self::Working => "Working",
            Self::Done => "Done",
            Self::Idle => "Idle",
            Self::Unknown => "Unknown",
            Self::NoStatus => "No status",
        }
    }

    /// Dot colour (SidebarSpec.dc.html:38 legend): red for Needs you and
    /// Error, yellow Working, green Done, the accent for Idle, and slate for
    /// no agent and no status. The legend has no Unknown; it is slate too
    /// (Wave 0a brief), so an agent with an unknown state looks like no
    /// status, and its row text tells them apart.
    pub(crate) const fn color(self) -> u32 {
        match self {
            Self::NeedsYou | Self::Error => color::RED,
            Self::Working => color::YELLOW,
            Self::Done => color::GREEN,
            Self::Idle => color::ACCENT,
            Self::Unknown | Self::NoStatus => color::SLATE,
        }
    }
}

/// Where an AGENTS row sorts (owner decision 2026-11-20, fork R27 agent
/// pins): Needs you, Error, the pinned agents, then Working, Done, Idle and
/// Unknown. A pin lifts an agent above the calm states only: a pinned agent
/// that needs you or failed stays in that group, so nothing hides an agent
/// that waits. NoStatus is never an agent (Q4) and sorts last for space dots.
/// The declaration order is the sort order; within one group the sidebar
/// keeps its receive order and tie-breaks (`sidebar::agent_order_in`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AgentGroup {
    NeedsYou,
    Error,
    Pinned,
    Working,
    Done,
    Idle,
    Unknown,
    NoStatus,
}

impl AgentGroup {
    /// The group of an agent with `status`, pinned or not.
    pub(crate) const fn of(status: SidebarStatus, pinned: bool) -> Self {
        match (status, pinned) {
            (SidebarStatus::NeedsYou, _) => Self::NeedsYou,
            (SidebarStatus::Error, _) => Self::Error,
            (SidebarStatus::NoStatus, _) => Self::NoStatus,
            (_, true) => Self::Pinned,
            (SidebarStatus::Working, false) => Self::Working,
            (SidebarStatus::Done, false) => Self::Done,
            (SidebarStatus::Idle, false) => Self::Idle,
            (SidebarStatus::Unknown, false) => Self::Unknown,
        }
    }
}

/// Herdr 1 maps to five values; Error and NoStatus come from L8.
pub(crate) fn from_agent_status(status: AgentStatus) -> SidebarStatus {
    match status {
        AgentStatus::Blocked => SidebarStatus::NeedsYou,
        AgentStatus::Working => SidebarStatus::Working,
        AgentStatus::Done => SidebarStatus::Done,
        AgentStatus::Idle => SidebarStatus::Idle,
        AgentStatus::Unknown => SidebarStatus::Unknown,
    }
}

#[cfg(test)]
#[path = "sidebar_status_tests.rs"]
mod tests;
