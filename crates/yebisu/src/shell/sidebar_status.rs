//! L3: the sidebar's own status type (S-8). Owner: L3.
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

    /// Dot colour (SidebarSpec legend); grey for no agent and no status.
    pub(crate) const fn color(self) -> u32 {
        match self {
            Self::NeedsYou | Self::Error => 0xbf616a,
            Self::Working => 0x88c0d0,
            Self::Done => 0xa3be8c,
            Self::Idle => 0x9ca3af,
            Self::Unknown => 0xb48ead,
            Self::NoStatus => 0x6b7280,
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
