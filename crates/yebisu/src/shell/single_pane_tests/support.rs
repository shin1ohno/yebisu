//! Fixtures: a connected mock host whose focused tab has two panes, a
//! measured terminal column, and a reader of the zoom requests on the wire.
use crate::{app::Smoke, mock_peer};
use gpui::{AppContext, Bounds, Entity, TestAppContext, point, px, size};
use sheprd_client::protocol::{ClientMessage, ClientShellSnapshot};
use std::{
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};

pub(super) const HOST: &str = "sandbox";

pub(super) struct Peer {
    pub(super) view: Entity<Smoke>,
    pub(super) receipts: Receiver<ClientMessage>,
}

pub(super) fn ready(view: &Entity<Smoke>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.terminal.navigating.is_none()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "peer timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Connects the recorded mock host (its first tab has two panes) and drops
/// everything the connection sent before the test starts.
pub(super) fn connected(cx: &mut TestAppContext) -> Peer {
    let view = cx.new(Smoke::new);
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer {e}"));
    view.update(cx, |this, _| {
        this.select_host(HOST);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    ready(&view, cx);
    let peer = Peer { view, receipts };
    assert_eq!(pane_count(&peer, cx), 2, "the mock tab has two panes");
    assert!(zoom_requests(&peer).is_empty());
    peer
}

fn pane_count(peer: &Peer, cx: &mut TestAppContext) -> usize {
    peer.view.update(cx, |this, _| {
        crate::board::snapshot(&this.terminal.live)
            .and_then(crate::shell::single_pane::focused_tab)
            .map_or(0, |tab| tab.panes)
    })
}

/// The terminal column measured at `width` points (what the last paint stores).
pub(super) fn measure(this: &mut Smoke, width: f32) {
    this.terminal.geometry.grid = Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(600.0)));
}

/// What the daemon's snapshot says about the focused tab's zoom.
pub(super) fn show_zoomed(this: &mut Smoke, zoomed: bool) {
    let Some(snapshot) = this.terminal.live.snapshot.clone() else {
        panic!("no snapshot");
    };
    let mut changed = (*snapshot).clone();
    for tab in &mut changed.tabs {
        tab.zoomed = zoomed;
    }
    this.terminal.live.pending_snapshot = None;
    this.terminal.live.snapshot = Some(Arc::new(changed));
}

/// The `pane.zoom` requests sent so far, waiting briefly for late ones.
pub(super) fn zoom_requests(peer: &Peer) -> Vec<serde_json::Value> {
    let mut found = Vec::new();
    while let Ok(message) = peer.receipts.recv_timeout(Duration::from_millis(150)) {
        let ClientMessage::ClientShellEndpointRequest { request, .. } = message else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&request) else {
            continue;
        };
        if value["method"] == "pane.zoom" {
            found.push(value);
        }
    }
    found
}

/// A snapshot whose focused tab (`w1:t1`) has `panes` panes.
pub(super) fn snapshot(panes: usize, zoomed: bool) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}"));
    let first = snapshot.panes[0].clone();
    snapshot.panes = (1..=panes)
        .map(|n| {
            let mut pane = first.clone();
            pane.pane_id = format!("w1:p{n}");
            pane
        })
        .collect();
    for tab in &mut snapshot.tabs {
        tab.zoomed = zoomed;
    }
    snapshot
}
