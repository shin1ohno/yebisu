use super::*;
use crate::shell::sidebar_status::from_agent_status;
use sheprd_client::protocol::AgentStatus;

#[test]
fn sidebar_status_has_seven_values() {
    let all = SidebarStatus::ALL;
    assert_eq!(all.len(), 7);
    let mut ranks = all.iter().map(|s| s.rank()).collect::<Vec<_>>();
    ranks.dedup();
    assert_eq!(ranks, [0, 1, 2, 3, 4, 5, 6]);
    let labels = all.iter().map(|s| s.label()).collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            "Needs you",
            "Error",
            "Working",
            "Done",
            "Idle",
            "Unknown",
            "No status"
        ]
    );
    // Error and NoStatus are values of their own, not folded into others.
    assert!(SidebarStatus::Error.is_agent());
    assert!(!SidebarStatus::NoStatus.is_agent());
}

#[test]
fn agent_status_maps_to_sidebar_status() {
    let mapped = [
        (AgentStatus::Blocked, SidebarStatus::NeedsYou),
        (AgentStatus::Working, SidebarStatus::Working),
        (AgentStatus::Done, SidebarStatus::Done),
        (AgentStatus::Idle, SidebarStatus::Idle),
        (AgentStatus::Unknown, SidebarStatus::Unknown),
    ];
    for (wire, ours) in mapped {
        assert_eq!(from_agent_status(wire), ours, "{wire:?}");
    }
    // Herdr 1 never yields Error or NoStatus (those come from Herdr 2, L8).
    let reached = mapped.map(|(wire, _)| from_agent_status(wire));
    assert!(!reached.contains(&SidebarStatus::Error));
    assert!(!reached.contains(&SidebarStatus::NoStatus));
}
