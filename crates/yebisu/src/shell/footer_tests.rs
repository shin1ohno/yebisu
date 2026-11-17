//! L2b tests: the rules of plan v2 7.6, one topic file each, and the
//! hardware-key routing of review cf8bbcccc2164abe F1 (S5).
use super::*;
use crate::{app::Smoke, mock_peer};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use sheprd_client::protocol::ClientMessage;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[path = "footer_tests/hosts.rs"]
mod hosts;
#[path = "footer_tests/items.rs"]
mod items;
#[path = "footer_tests/keys.rs"]
mod keys;
#[path = "footer_tests/new_space.rs"]
mod new_space;
#[path = "footer_tests/palette.rs"]
mod palette;
#[path = "footer_tests/status.rs"]
mod status;

fn window(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    cx.add_window_view(|_, cx| Smoke::new(cx))
}

/// A window whose terminal is connected to the recorded mock peer and drawn
/// once (as `sh1_156_tap_tests` does).
fn connected(
    cx: &mut TestAppContext,
) -> (
    Entity<Smoke>,
    &mut VisualTestContext,
    mpsc::Receiver<ClientMessage>,
) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, wire) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.surface.is_some() && this.handoff.stamp.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "L2b frame timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx, wire)
}

/// Every message the peer received until `quiet` passes with nothing new.
fn drain(wire: &mpsc::Receiver<ClientMessage>, quiet: Duration) -> Vec<ClientMessage> {
    let mut out = Vec::new();
    while let Ok(message) = wire.recv_timeout(quiet) {
        out.push(message);
    }
    out
}
