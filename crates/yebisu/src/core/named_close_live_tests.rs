//! Close Space on a connected `space-<ms>` row (#249), against real
//! herdr-core servers: the private fleet of `space_fleet.rs`, one server per
//! space, each started and ended only by the test, stopped through the
//! production stop command (`vacate.rs`) run against the fleet's private
//! directories. Opt in like `empty_space_tests`:
//! `YEBISU_HERDR_CORE_TEST_BINARY=... cargo test -p yebisu named_close_live
//! -- --ignored --test-threads=1`.
use super::{
    empty_space_tests::{connect, discover, moved_to, prepare_next, status, throughout, until},
    named_close::STARTED_SINCE,
    running,
    space_fleet::Fleet,
};
use crate::{
    app::Smoke,
    shell::{
        host_menus::{MenuItemId, SpaceMenuItem},
        sidebar::long_press::PressedRow,
        space_close::OPEN_TO_CLOSE,
    },
};
use gpui::{Entity, TestAppContext, VisualTestContext, point, px};
use sheprd_core_client::wire::TerminalState;

const CLOSE: MenuItemId = MenuItemId::Space(SpaceMenuItem::Close);

/// What the status poll would have found in `space` (`running::busy` over
/// a fresh list), put where the view reads it.
fn poll_busy(view: &Entity<Smoke>, cx: &mut VisualTestContext, fleet: &Fleet, space: &str) {
    let busy = running::busy(&fleet.list(space));
    publish_busy(view, cx, busy);
}

fn publish_busy(view: &Entity<Smoke>, cx: &mut VisualTestContext, busy: Vec<String>) {
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("no Herdr 2 session"))
            .statuses_mailbox()
            .publish_busy(busy);
    });
}

/// The long-press menu of `space`'s row: Close's state and reason.
fn close_entry(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    host: &str,
    space: &str,
) -> (bool, Option<&'static str>) {
    view.update(cx, |this, _| {
        this.sidebar_long_press(
            PressedRow::Space {
                host: host.into(),
                id: space.into(),
                name: space.into(),
            },
            point(px(40.), px(200.)),
        );
        let menu = this
            .shell
            .sidebar
            .long_press
            .clone()
            .unwrap_or_else(|| panic!("no menu for {space}"));
        let close = menu
            .entries
            .iter()
            .find(|e| e.item == CLOSE)
            .unwrap_or_else(|| panic!("no Close Space for {space}"));
        (close.enabled, close.reason)
    })
}

/// Picks Close Space from the open menu.
fn pick_close(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> crate::Result<()> {
    cx.update(|window, cx| view.update(cx, |this, cx| this.pick_long_press(CLOSE, window, cx)))
}

fn asked(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |this, _| this.shell.sidebar.close_confirm.is_some())
}

/// A shells-only `space-<ms>` closes at once from its own row's menu: its
/// server, and only it, is stopped through the checked stop, and the view
/// moves to the next space. `default`'s row and another session's row
/// cannot close, each with its own reason.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn named_close_shells_only_stops_only_that_server(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "space-1", "space-2"] {
        fleet.add(space);
    }
    let host = "named-close";
    let (view, cx) = connect(&mut fleet, host, "space-1", cx);
    discover(&view, cx, &fleet, host);
    prepare_next(&view, cx, &mut fleet, "default");
    poll_busy(&view, cx, &fleet, "space-1");
    assert_eq!(
        close_entry(&view, cx, host, "default"),
        (false, Some(crate::core::NAMED_CLOSE_PROTECTED))
    );
    assert_eq!(
        close_entry(&view, cx, host, "space-2"),
        (false, Some(OPEN_TO_CLOSE))
    );
    assert_eq!(close_entry(&view, cx, host, "space-1"), (true, None));
    let id = fleet.core_id("space-1");
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(!asked(&view, cx), "a shell alone closes at once");
    until(&view, cx, "space-1's server stops", 15, |_| {
        !fleet.alive("space-1")
    });
    assert!(
        fleet.alive("space-2"),
        "another space's server keeps running"
    );
    assert!(fleet.alive("default"), "the default space keeps running");
    assert_eq!(fleet.stops(), vec![("space-1".to_owned(), id)]);
    until(&view, cx, "selection moves", 15, |cx| {
        moved_to(&view, cx, "default")
    });
}

/// A program another client runs in the space makes Close ask; one the
/// question did not name (the poll had not seen it) refuses the close and
/// ends nothing. Confirming the question ends it and stops the server.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn named_close_asks_for_a_program_and_refuses_one_not_asked(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "space-1"] {
        fleet.add(space);
    }
    let host = "named-close-busy";
    let (view, cx) = connect(&mut fleet, host, "space-1", cx);
    discover(&view, cx, &fleet, host);
    prepare_next(&view, cx, &mut fleet, "default");
    let sleep = fleet.run_elsewhere("space-1");
    // The poll's last list is older than the program: it names nothing.
    publish_busy(&view, cx, Vec::new());
    assert_eq!(close_entry(&view, cx, host, "space-1"), (true, None));
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(!asked(&view, cx), "the stale list said shells alone");
    until(&view, cx, "the refusal is shown", 10, |cx| {
        status(&view, cx).contains(STARTED_SINCE)
    });
    throughout(&view, cx, "nothing ends", 2, |_| {
        fleet.alive("space-1")
            && fleet
                .list("space-1")
                .iter()
                .any(|t| t.address == sleep && t.state == TerminalState::Live)
    });
    assert!(fleet.stops().is_empty(), "no stop was asked");
    // A fresh list names the program: Close asks, and the answer ends it.
    poll_busy(&view, cx, &fleet, "space-1");
    let id = fleet.core_id("space-1");
    assert_eq!(close_entry(&view, cx, host, "space-1"), (true, None));
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(asked(&view, cx), "a running program asks first");
    view.update(cx, |this, _| this.confirm_space_close())
        .unwrap_or_else(|e| panic!("confirm: {e}"));
    until(&view, cx, "space-1's server stops", 20, |_| {
        !fleet.alive("space-1")
    });
    assert!(fleet.alive("default"), "the default space keeps running");
    assert_eq!(fleet.stops(), vec![("space-1".to_owned(), id)]);
    until(&view, cx, "selection moves", 15, |cx| {
        moved_to(&view, cx, "default")
    });
}
