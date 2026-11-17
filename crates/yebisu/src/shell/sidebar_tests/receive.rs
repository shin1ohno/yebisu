use super::*;
use crate::{app::Smoke, live::Live};
use gpui::AppContext;
use sheprd_client::{ClientEvent, protocol::AgentStatus};
use std::sync::Arc;

/// A connected reader whose one agent (pane w1:p1) has `status` and `seq`.
fn reader(status: AgentStatus, seq: u64) -> Live {
    let mut snap = fixture();
    snap.agents[0].agent_status = status;
    snap.agents[0].state_change_seq = seq;
    let mut live = Live::default();
    live.connected = true;
    live.apply(ClientEvent::Snapshot(Arc::new(snap)));
    live
}

fn id(host: &str) -> AgentIdentity {
    AgentIdentity {
        host: host.into(),
        kind: HerdrKind::Herdr1,
        pane: "w1:p1".into(),
    }
}

/// Review SF-1: the receive order is taken when a reader update arrives
/// (`record_board`, which runs for every host's update), not when the list
/// is drawn. Nothing here renders the sidebar.
#[gpui::test]
fn receive_order_is_recorded_when_updates_arrive(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        // First batches of two hosts, then a changes before b.
        this.record_board("a", &reader(AgentStatus::Idle, 1));
        this.record_board("b", &reader(AgentStatus::Idle, 1));
        this.record_board("a", &reader(AgentStatus::Working, 2));
        this.record_board("b", &reader(AgentStatus::Working, 2));
        let sidebar = &this.shell.sidebar;
        let (a, b) = (sidebar.received(&id("a")), sidebar.received(&id("b")));
        assert!(a > 0, "a's change was not recorded on arrival");
        assert!(b > a, "b changed last but is not newer ({b} <= {a})");
        // A disconnect update forgets the host: its next snapshot is a first batch.
        let mut gone = reader(AgentStatus::Working, 2);
        gone.connected = false;
        this.record_board("a", &gone);
        this.record_board("a", &reader(AgentStatus::Done, 3));
        assert_eq!(this.shell.sidebar.received(&id("a")), 0);
    });
}
