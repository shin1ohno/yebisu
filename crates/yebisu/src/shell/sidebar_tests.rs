//! L3 sidebar tests (plan v2 7.8), one file per topic.
use super::*;

#[path = "sidebar_tests/forward.rs"]
mod forward;
#[path = "sidebar_tests/identity.rs"]
mod identity;
#[path = "sidebar_tests/long_press.rs"]
mod long_press;
#[path = "sidebar_tests/order.rs"]
mod order;
#[path = "sidebar_tests/receive.rs"]
mod receive;
#[path = "sidebar_tests/rows.rs"]
mod rows;
#[path = "sidebar_tests/status.rs"]
mod status;
#[path = "sidebar_tests/tap.rs"]
mod tap;
#[path = "sidebar_tests/wave1.rs"]
mod wave1;

/// A Herdr 1 snapshot fixture with one space, one tab and one agent.
fn fixture() -> sheprd_client::protocol::ClientShellSnapshot {
    serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"))
}

fn row(host: &str, pane: &str, name: &str, status: SidebarStatus, received: u64) -> AgentRow {
    AgentRow {
        identity: AgentIdentity {
            host: host.into(),
            kind: HerdrKind::Herdr1,
            pane: pane.into(),
        },
        name: name.into(),
        tab: 1,
        status,
        received,
        icon: agent_icon::AgentIcon::Generic,
    }
}

fn names(rows: &[AgentRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}
