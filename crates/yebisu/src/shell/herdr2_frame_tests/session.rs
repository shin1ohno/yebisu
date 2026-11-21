use super::*;
use crate::shell::sidebar::{accessibility_name, model::agents_list};

fn agent_names(host: &crate::shell::sidebar::model::HostEntry) -> Vec<String> {
    host.agents
        .iter()
        .map(|l| l.row.identity.pane.clone())
        .collect()
}

#[test]
fn default_session_keeps_one_space_row_on_upgrade() {
    let state = SidebarState::default();
    let statuses = [
        raw("t1", Some("working")),
        raw("t2", None),
        raw("t3", Some("done")),
    ];
    let host = connected_host("pro-dev", &statuses, &state);
    assert_eq!(host.spaces.len(), 1, "one session is one space row");
    let space = &host.spaces[0];
    assert_eq!(space.id, HERDR2_SESSION);
    assert_eq!(space.name, HERDR2_SESSION);
    assert_eq!(space.tabs, 1, "one session is one tab");
    assert!(space.current);
    // Every agent of the session sits in that one tab.
    assert!(host.agents.iter().all(|line| line.row.tab == 1));
    // Not connected: no session row is kept (decided 2026-11-11).
    let offline = host_entry(
        "pro-dev",
        HostLink::Offline(crate::shell::sidebar::model::OfflineReason::NotConnected),
        &statuses,
        &state,
    );
    assert!(offline.spaces.is_empty() && offline.agents.is_empty());
}

#[test]
fn herdr2_status_less_not_in_agents() {
    let state = SidebarState::default();
    let host = connected_host(
        "pro-dev",
        &[raw("shell", None), raw("agent", Some("working"))],
        &state,
    );
    assert_eq!(agent_names(&host), ["agent"]);
    assert_eq!(
        agents_list(std::slice::from_ref(&host))
            .iter()
            .map(|l| l.row.identity.pane.as_str())
            .collect::<Vec<_>>(),
        ["agent"]
    );
    // Only status-less terminals: the session row says No agent.
    let quiet = connected_host("quiet", &[raw("a", None), raw("b", None)], &state);
    assert!(quiet.agents.is_empty());
    assert_eq!(quiet.spaces[0].detail, "No agent");
    assert_eq!(quiet.spaces[0].dot, SidebarStatus::NoStatus);
}

#[test]
fn herdr2_error_after_needs_you() {
    let state = SidebarState::default();
    let host = connected_host(
        "pro-dev",
        &[
            raw("w", Some("working")),
            raw("u", Some("mystery")),
            raw("d", Some("done")),
            raw("e", Some("error")),
            raw("i", Some("idle")),
            raw("b", Some("blocked")),
        ],
        &state,
    );
    let order = agents_list(std::slice::from_ref(&host))
        .iter()
        .map(|l| l.row.status)
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [
            SidebarStatus::NeedsYou,
            SidebarStatus::Error,
            SidebarStatus::Working,
            SidebarStatus::Done,
            SidebarStatus::Idle,
            SidebarStatus::Unknown,
        ]
    );
    // The space row carries one Needs you and one Error tag (q3).
    assert_eq!(host.spaces[0].tags(), ["Needs you", "Error"]);
}

#[test]
fn herdr2_rows_accessibility_name_includes_kind() {
    let state = SidebarState::default();
    let host = connected_host("pro-dev", &[raw("t2", Some("error"))], &state);
    let line = &host.agents[0];
    assert_eq!(line.row.identity.kind, HerdrKind::Herdr2);
    let spoken = accessibility_name(&line.row);
    assert!(spoken.contains("Herdr 2"), "{spoken}");
    assert!(spoken.contains("pane t2"), "{spoken}");
    assert_eq!(host.detail(), "SSH · Herdr 2");
}

#[test]
fn herdr2_row_text_is_one_line() {
    let state = SidebarState::default();
    let mut status = raw("t1", Some("working"));
    if let Some(s) = status.status.as_mut() {
        s.title = "evil\ntitle\r".into();
    }
    let host = connected_host("pro-dev", &[status], &state);
    assert_eq!(host.agents[0].row.name, "evil title");
}
