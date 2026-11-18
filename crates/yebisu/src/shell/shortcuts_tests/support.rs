//! Shared fixtures for the Shortcuts tests.
use crate::{live::Live, mock_peer, shell::ChromeState, terminal_view::TerminalView};
use sheprd_client::ClientEvent;
use std::sync::Arc;

/// A connected Herdr 1 terminal with a snapshot, a surface and `methods`.
pub(super) fn terminal(methods: &[&str]) -> TerminalView {
    let mut live = Live::default();
    live.connected = true;
    live.methods = methods.iter().map(|m| (*m).to_owned()).collect();
    live.apply(ClientEvent::Snapshot(Arc::new(
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}")),
    )));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    TerminalView {
        live,
        open: true,
        ..Default::default()
    }
}

pub(super) fn connected() -> ChromeState {
    ChromeState {
        connected: true,
        find_available: true,
        ..ChromeState::default()
    }
}
