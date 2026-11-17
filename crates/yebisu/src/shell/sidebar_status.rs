//! L3: the sidebar's own status type (S-8). Owner: L3.
use sheprd_client::protocol::AgentStatus;

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

/// Herdr 1 maps to five values; Error and NoStatus come from L8.
pub(crate) fn from_agent_status(_status: AgentStatus) -> SidebarStatus {
    SidebarStatus::Unknown
}
