//! L4 filter rules, one topic per file. Names follow plan v2 7.9.
use super::*;
use crate::shell::{
    HerdrKind,
    sidebar::{AgentIdentity, AgentRow},
    sidebar_status::SidebarStatus,
};

#[path = "filter_tests/empty.rs"]
mod empty;
#[path = "filter_tests/rows.rs"]
mod rows;
#[path = "filter_tests/screen.rs"]
mod screen;

pub(super) fn agent(host: &str, name: &str, pane: &str) -> AgentRow {
    AgentRow {
        identity: AgentIdentity {
            host: host.to_owned(),
            kind: HerdrKind::Herdr1,
            pane: pane.to_owned(),
        },
        name: name.to_owned(),
        tab: 1,
        status: SidebarStatus::Idle,
        received: 0,
    }
}

pub(super) fn fleet() -> Vec<AgentRow> {
    vec![
        agent("pro-dev", "claude", "p1"),
        agent("pro-dev", "Codex", "p2"),
        agent("mini", "claude", "p3"),
    ]
}
