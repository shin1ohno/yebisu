//! A merged row keeps the connection it was drawn from (review of PR 90,
//! finding 3): a tap from a row drawn before a reconnect is rejected exactly
//! as the Herdr 1 space row's is today, even when the new connection has a
//! space with the same ID.
use super::*;
use crate::{
    app::Smoke,
    board::snapshot,
    shell::sidebar::model::{SpaceInput, space_entry},
};
use gpui::AppContext as _;
use std::time::{Duration, Instant};

/// Connects host `A` to a fresh mock peer and waits until its reader holds a
/// snapshot. Returns the link generation of the new connection and the
/// peer's record of the requests it received.
fn connect(
    view: &gpui::Entity<Smoke>,
    cx: &mut gpui::TestAppContext,
) -> (
    u64,
    std::sync::mpsc::Receiver<sheprd_client::protocol::ClientMessage>,
) {
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    let generation = view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        generation
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let ready = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal
                .link
                .inbox
                .lock()
                .map(|live| snapshot(&live).is_some())
                .unwrap_or(false)
        });
        if ready {
            return (generation, receipts);
        }
        assert!(Instant::now() < deadline, "no snapshot");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The merged rows of host `A` as the sidebar would draw them now: one
/// Herdr 1 connection with workspace `w1`, stamped with the reader's boot ID
/// and the link generation (`sidebar::view::host_entries`).
fn drawn_space(view: &gpui::Entity<Smoke>, cx: &mut gpui::TestAppContext) -> ListSpace {
    let (boot, generation) = view.update(cx, |this, _| {
        let live = this
            .terminal
            .link
            .inbox
            .lock()
            .unwrap_or_else(|e| panic!("lock: {e}"));
        let boot = snapshot(&live)
            .map(|s| s.boot_id.clone())
            .unwrap_or_else(|| panic!("snapshot"));
        (boot, this.terminal.link.generation)
    });
    let mut entry = HostEntry::new("A", HerdrKind::Herdr1, HostLink::Connected);
    entry.boot = boot;
    entry.generation = generation;
    let w1 = space_entry(
        HerdrKind::Herdr1,
        SpaceInput {
            id: "w1".into(),
            name: "w1".into(),
            tabs: 1,
            branch: None,
            current: false,
        },
        &[],
    );
    let rows = merge(&[entry.with_content(vec![w1], Vec::new())]);
    match rows.as_slice() {
        [ListRow::Host(_), ListRow::Space(space)] => space.clone(),
        other => panic!("rows: {other:?}"),
    }
}

#[gpui::test]
fn a_row_drawn_before_a_reconnect_to_the_same_workspace_id_is_rejected(
    cx: &mut gpui::TestAppContext,
) {
    let view = cx.new(Smoke::new);
    let (first, _old_peer) = connect(&view, cx);
    let stale = drawn_space(&view, cx);
    assert_eq!(stale.generation, first);
    assert_eq!(stale.boot, "boot-v1");

    // Reconnect to the same daemon: the same boot ID and the same workspace
    // `w1`; only the link generation moves on.
    let (second, receipts) = connect(&view, cx);
    assert_ne!(first, second);
    let fresh = drawn_space(&view, cx);
    assert_eq!(fresh.entry.id, stale.entry.id);
    assert_eq!(fresh.boot, stale.boot);
    assert_eq!(fresh.generation, second);
    assert_ne!(stale.tap(), fresh.tap());

    let refused = view.update(cx, |this, _| this.sidebar_tap(stale.tap()));
    match refused {
        Err(e) => assert!(e.to_string().contains("Stale board row"), "{e}"),
        Ok(()) => panic!("a row from the old connection was accepted"),
    }
    assert!(
        receipts.recv_timeout(Duration::from_millis(150)).is_err(),
        "the stale row sent a request"
    );

    // The same space drawn from the new connection still opens.
    let opened = view.update(cx, |this, _| this.sidebar_tap(fresh.tap()));
    assert!(opened.is_ok(), "{opened:?}");
}
