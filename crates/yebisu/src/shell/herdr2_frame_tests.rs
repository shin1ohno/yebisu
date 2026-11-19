//! L8 tests (plan v2 7.14), one file per topic.
use super::{session as rows, *};
use crate::shell::{
    ChromeState,
    sidebar::{SidebarState, model::HostLink},
};

#[path = "herdr2_frame_tests/chrome.rs"]
mod chrome;
#[path = "herdr2_frame_tests/connection.rs"]
mod connection;
#[path = "herdr2_frame_tests/entry.rs"]
mod entry;
#[path = "herdr2_frame_tests/panes.rs"]
mod panes_tests;
#[path = "herdr2_frame_tests/session.rs"]
mod session;
#[path = "herdr2_frame_tests/sidebar.rs"]
mod sidebar;
#[path = "herdr2_frame_tests/status.rs"]
mod status;

/// A raw status as herdr-core sends it; `None` is a terminal whose program
/// reported nothing.
fn raw(address: &str, state: Option<&str>) -> CoreStatus {
    let status = state.map(|state| {
        serde_json::from_value(serde_json::json!({
            "app": "claude",
            "state": state,
            "title": format!("agent {address}"),
        }))
        .unwrap_or_else(|e| panic!("wire status {state}: {e}"))
    });
    CoreStatus {
        address: address.into(),
        status,
    }
}

fn herdr2(connected: bool) -> ChromeState {
    ChromeState {
        kind: HerdrKind::Herdr2,
        connected,
        ..ChromeState::default()
    }
}

/// A connected Herdr 2 host with these terminals, as the sidebar draws it.
fn connected_host(
    host: &str,
    statuses: &[CoreStatus],
    state: &SidebarState,
) -> crate::shell::sidebar::model::HostEntry {
    rows::host_entry(host, HostLink::Connected, 1, statuses, state)
}
