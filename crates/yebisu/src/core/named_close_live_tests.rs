//! Close Space on a connected `space-<ms>` row (#249), against real
//! herdr-core servers: the private fleet of `space_fleet.rs`, one server per
//! space, each started and ended only by the test, stopped through the
//! production stop command (`vacate.rs`) run against the fleet's private
//! directories. Opt in like `empty_space_tests`:
//! `YEBISU_HERDR_CORE_TEST_BINARY=... cargo test -p yebisu named_close_live
//! -- --ignored --test-threads=1`.
use super::{
    empty_space_tests::{connect, discover, moved_to, prepare_next, status, throughout, until},
    named_close::{JUST_STARTED, STARTED_SINCE},
    running,
    space_fleet::Fleet,
};
use crate::{
    app::Smoke,
    shell::{
        host_menus::{MenuItemId, SpaceMenuItem},
        sidebar::long_press::PressedRow,
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
/// moves to the next space. `default`'s row cannot close, and another
/// session's row, seen by the poll only now, waits `vacate::GRACE`.
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
        (false, Some(JUST_STARTED))
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

/// The view's selected host's catalog lists `space`.
fn lists(view: &Entity<Smoke>, cx: &mut VisualTestContext, space: &str) -> bool {
    view.update(cx, |this, _| {
        let epoch = this.terminal.live.input_epoch;
        this.terminal
            .core
            .as_ref()
            .is_some_and(|core| core.sessions(epoch).iter().any(|row| row.name == space))
    })
}

/// #249 B: a `space-<ms>` row this connection does not serve closes from
/// the discovery poll's snapshot. A server first seen just now waits
/// `GRACE`; a shell whose foreground runs a program asks; a program started
/// after the question refuses the stop and ends nothing; confirming again
/// with a fresh snapshot ends both and stops only that server.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn listed_close_asks_waits_grace_and_refuses_an_unseen_program(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "space-1", "space-2"] {
        fleet.add(space);
    }
    let host = "listed-close";
    let (view, cx) = connect(&mut fleet, host, "space-1", cx);
    // A shell at its prompt runs nothing; its foreground child does (an
    // interactive shell gives it its own process group, as a terminal's
    // shell does).
    let shell = fleet.run_argv_elsewhere("space-2", &["sh", "-ic", "sleep 600; :"]);
    discover(&view, cx, &fleet, host);
    assert_eq!(
        close_entry(&view, cx, host, "space-2"),
        (false, Some(JUST_STARTED))
    );
    std::thread::sleep(super::vacate::GRACE + std::time::Duration::from_millis(200));
    discover(&view, cx, &fleet, host);
    assert_eq!(close_entry(&view, cx, host, "space-2"), (true, None));
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    let question = view.read_with(cx, |this, _| {
        this.shell.sidebar.close_confirm.clone().map(|c| c.question)
    });
    assert_eq!(
        question.map(|q| q.running),
        Some(crate::shell::space_close::Running::Programs(1)),
        "the shell's foreground sleep asks"
    );
    // Another client starts a program after the question was asked.
    let later = fleet.run_elsewhere("space-2");
    view.update(cx, |this, _| this.confirm_space_close())
        .unwrap_or_else(|e| panic!("confirm: {e}"));
    until(&view, cx, "the refusal is shown", 15, |cx| {
        status(&view, cx).contains(STARTED_SINCE)
    });
    let live = |fleet: &Fleet, address: &str| {
        fleet
            .list("space-2")
            .iter()
            .any(|t| t.address == address && t.state == TerminalState::Live)
    };
    assert!(fleet.alive("space-2"), "the stop was refused");
    assert!(
        live(&fleet, &shell) && live(&fleet, &later),
        "nothing ended"
    );
    // A fresh snapshot names both: Close asks for 2 and the stop ends them.
    discover(&view, cx, &fleet, host);
    let id = fleet.core_id("space-2");
    assert_eq!(close_entry(&view, cx, host, "space-2"), (true, None));
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(asked(&view, cx), "two programs ask first");
    view.update(cx, |this, _| this.confirm_space_close())
        .unwrap_or_else(|e| panic!("confirm: {e}"));
    until(&view, cx, "space-2's server stops", 20, |_| {
        !fleet.alive("space-2")
    });
    assert!(fleet.alive("space-1"), "the connected space keeps running");
    assert!(fleet.alive("default"), "the default space keeps running");
    let stops = fleet.stops();
    assert_eq!(stops.len(), 2, "{stops:?}");
    assert!(
        stops
            .iter()
            .all(|stop| *stop == ("space-2".to_owned(), id.clone()))
    );
    until(&view, cx, "its row leaves the sidebar", 10, |cx| {
        !lists(&view, cx, "space-2")
    });
    assert!(
        moved_to(&view, cx, "space-1"),
        "the view stays where it was"
    );
}

/// The foreground of `space`'s terminal `address`, as `hc ls` shows it:
/// its pid and name.
fn foreground(fleet: &Fleet, space: &str, address: &str) -> Option<(i32, String)> {
    fleet
        .list(space)
        .iter()
        .find(|t| t.address == address)
        .and_then(|t| t.foreground.as_ref().map(|fg| (fg.pid, fg.name.clone())))
}

/// Waits up to 10 s until `space`'s terminal `address` has a foreground
/// for which `done` holds.
#[track_caller]
fn wait_foreground(
    fleet: &Fleet,
    space: &str,
    address: &str,
    what: &str,
    done: impl Fn(&(i32, String)) -> bool,
) -> (i32, String) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(found) = foreground(fleet, space, address).filter(|fg| done(fg)) {
            return found;
        }
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Review of #255: a program started in a terminal the snapshot saw idle
/// (another client types into its shell between the poll and Close) was
/// never in the question, so the stop refuses it and ends nothing; a fresh
/// snapshot asks about it, and the answer ends it.
#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn listed_close_refuses_a_program_started_in_a_seen_shell(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "space-1", "space-2"] {
        fleet.add(space);
    }
    let host = "listed-close-changed";
    let (view, cx) = connect(&mut fleet, host, "space-1", cx);
    // An interactive shell at its prompt: nothing to ask about.
    let shell = fleet.run_argv_elsewhere("space-2", &["sh", "-i"]);
    // The shell itself in the foreground, once it is reported.
    let (idle, _) = wait_foreground(&fleet, "space-2", &shell, "the shell", |_| true);
    discover(&view, cx, &fleet, host);
    std::thread::sleep(super::vacate::GRACE + std::time::Duration::from_millis(200));
    discover(&view, cx, &fleet, host);
    // Another client starts a program in that shell after the snapshot.
    fleet.send_elsewhere("space-2", &shell, "sleep 600");
    let (program, _) = wait_foreground(&fleet, "space-2", &shell, "sleep never ran", |fg| {
        fg.0 != idle && fg.1 == "sleep"
    });
    assert_eq!(close_entry(&view, cx, host, "space-2"), (true, None));
    pick_close(&view, cx).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(!asked(&view, cx), "the snapshot saw a shell alone");
    until(&view, cx, "the refusal is shown", 15, |cx| {
        status(&view, cx).contains(STARTED_SINCE)
    });
    assert!(fleet.alive("space-2"), "the stop was refused");
    assert_eq!(
        foreground(&fleet, "space-2", &shell).map(|fg| fg.0),
        Some(program),
        "the program still runs"
    );
    // A fresh snapshot names it: Close asks, and the answer stops the space.
    discover(&view, cx, &fleet, host);
    pick_close_after_menu(&view, cx, host, "space-2");
    assert!(asked(&view, cx), "the program asks first");
    view.update(cx, |this, _| this.confirm_space_close())
        .unwrap_or_else(|e| panic!("confirm: {e}"));
    until(&view, cx, "space-2's server stops", 20, |_| {
        !fleet.alive("space-2")
    });
    assert!(fleet.alive("space-1") && fleet.alive("default"));
}

fn pick_close_after_menu(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    host: &str,
    space: &str,
) {
    assert_eq!(close_entry(view, cx, host, space), (true, None));
    pick_close(view, cx).unwrap_or_else(|e| panic!("close: {e}"));
}
