//! A Herdr 2 space whose last tab closes goes away with its server, against
//! real herdr-core servers: a private fleet of one server per space (`default`
//! and named spaces), each started and ended only by the test, stopped through
//! the production stop command run against the fleet's private directories.
//! Opt in like `live_core_tests`: `YEBISU_HERDR_CORE_TEST_BINARY=... cargo test
//! -p yebisu empty_space -- --ignored --test-threads=1`.
use super::{
    Command, SessionList, due_vacancies,
    live_core_tests::{command, control_d, panes, seen, tabs, wait},
    record_discovery_at, retire_vacant,
    space_fleet::Fleet,
    vacate::{Expect, Stopper, set_test_stopper},
};
use crate::app::Smoke;
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Connects a view to `space`'s server and lets it know its host's other
/// spaces, as the discovery poll does on the iPad.
fn connect<'a>(
    fleet: &mut Fleet,
    host: &str,
    space: &str,
    cx: &'a mut TestAppContext,
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    let stopper = fleet.stopper();
    connect_with(fleet, host, space, stopper, cx)
}

/// `connect` with the stopper the session's host will use.
fn connect_with<'a>(
    fleet: &mut Fleet,
    host: &str,
    space: &str,
    stopper: Stopper,
    cx: &'a mut TestAppContext,
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    set_test_stopper(host, stopper);
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    let backend = fleet.bridge(space);
    let (host, space) = (host.to_owned(), space.to_owned());
    view.update(cx, |this, cx| {
        this.select_host(&host);
        this.terminal.core_test_backend = Some(backend);
        let _ = this.connect_named_core(host, space, None, cx);
    });
    wait(&view, cx, "first pane", |live| {
        live.connected && panes(live) == 1
    });
    (view, cx)
}

fn discover(view: &Entity<Smoke>, cx: &mut VisualTestContext, fleet: &Fleet, host: &str) {
    let output = fleet.discover();
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("no Herdr 2 session"))
            .publish_discovery(None, host, &output)
            .unwrap_or_else(|e| panic!("discovery: {e}"));
    });
}

/// Where the view goes when its space is removed: a bridge to `space`, ready
/// for the connection the view then makes.
fn prepare_next(view: &Entity<Smoke>, cx: &mut VisualTestContext, fleet: &mut Fleet, space: &str) {
    let backend = fleet.bridge(space);
    view.update(cx, |this, _| {
        this.terminal.core_test_backend = Some(backend)
    });
}

/// Polls the view until `done`, for at most `secs`.
#[track_caller]
fn until(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    secs: u64,
    mut done: impl FnMut(&mut VisualTestContext) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !done(cx) {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        assert!(Instant::now() < deadline, "{what}: {}", seen(view, cx));
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Polls the view for `secs`, asserting `holds` every time.
#[track_caller]
fn throughout(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    secs: u64,
    mut holds: impl FnMut(&mut VisualTestContext) -> bool,
) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        assert!(holds(cx), "{what}: {}", seen(view, cx));
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The view's own session and what it lists for it.
fn own(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<(String, usize)> {
    view.update(cx, |this, _| {
        let epoch = this.terminal.live.input_epoch;
        let core = this.terminal.core.as_ref()?;
        let name = core.name().to_owned();
        let tabs = core
            .sessions(epoch)
            .into_iter()
            .find(|row| row.name == name)?
            .tabs
            .len();
        Some((name, tabs))
    })
}

fn status(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.update(cx, |this, _| this.terminal.live.status.clone())
}

fn selected(view: &Entity<Smoke>, cx: &mut VisualTestContext, host: &str) -> String {
    view.update(cx, |this, _| {
        this.shell.herdr2.selected_session(host).to_owned()
    })
}

fn close_the_tab(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let tab = tabs(view, cx)
        .first()
        .unwrap_or_else(|| panic!("tab not listed"))
        .id
        .clone();
    command(view, cx, Command::CloseTab(tab));
}

/// The view moved to `next`'s server and shows it.
fn moved_to(view: &Entity<Smoke>, cx: &mut VisualTestContext, next: &str) -> bool {
    view.update(cx, |this, _| {
        this.terminal.live.connected
            && this
                .terminal
                .core
                .as_ref()
                .is_some_and(|core| core.name() == next)
    })
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_last_tab_closed_by_x_stops_only_that_server(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "s1", "s2"] {
        fleet.add(space);
    }
    let host = "x-closes";
    let (view, cx) = connect(&mut fleet, host, "s1", cx);
    discover(&view, cx, &fleet, host);
    prepare_next(&view, cx, &mut fleet, "default");
    let id = fleet.core_id("s1");
    close_the_tab(&view, cx);
    until(&view, cx, "s1's server stops", 15, |_| !fleet.alive("s1"));
    assert!(fleet.alive("s2"), "another space's server keeps running");
    assert!(fleet.alive("default"), "the default space keeps running");
    assert_eq!(fleet.stops(), vec![("s1".to_owned(), id)]);
    // The selected space went: selection takes the place of the removed one
    // (the next row in the sidebar's order: default), never the removed one.
    until(&view, cx, "selection moves", 15, |cx| {
        moved_to(&view, cx, "default")
    });
    assert_eq!(selected(&view, cx, host), "default");
    assert!(
        fleet.live("s2") == 0,
        "nothing else was started or ended in s2"
    );
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_last_program_exit_stops_only_that_server(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "s1", "s2"] {
        fleet.add(space);
    }
    let host = "exits";
    let (view, cx) = connect(&mut fleet, host, "s1", cx);
    discover(&view, cx, &fleet, host);
    prepare_next(&view, cx, &mut fleet, "default");
    control_d(&view, cx);
    until(&view, cx, "s1's server stops", 15, |_| !fleet.alive("s1"));
    assert!(fleet.alive("s2"));
    assert!(fleet.alive("default"));
    assert_eq!(fleet.stops().len(), 1);
    until(&view, cx, "selection moves", 15, |cx| {
        moved_to(&view, cx, "default")
    });
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_with_no_other_space_detaches_and_forgets_the_selection(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    fleet.add("s1");
    let host = "alone";
    let (view, cx) = connect(&mut fleet, host, "s1", cx);
    discover(&view, cx, &fleet, host);
    view.update(cx, |this, _| {
        this.shell.herdr2.confirm_session(host, "s1");
    });
    close_the_tab(&view, cx);
    until(&view, cx, "s1's server stops", 15, |_| !fleet.alive("s1"));
    until(&view, cx, "the view detaches", 15, |cx| {
        view.update(cx, |this, _| this.terminal.core.is_none())
    });
    assert!(!view.update(cx, |this, _| this.terminal.live.connected));
    // A reconnect now goes to the host's default space, never to s1 again
    // (which would start a new empty server).
    assert_eq!(selected(&view, cx, host), "default");
    assert!(!fleet.alive("s1"));
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_with_a_terminal_outside_any_tab_is_not_stopped(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let host = "outside";
    let (view, cx) = connect(&mut fleet, host, "s1", cx);
    let outside = fleet.run_elsewhere("s1");
    close_the_tab(&view, cx);
    wait(&view, cx, "no tabs", |live| {
        live.connected && live.status.contains("No tabs")
    });
    throughout(&view, cx, "the space stays with its terminal", 3, |_| {
        fleet.alive("s1")
    });
    assert!(fleet.stops().is_empty(), "no stop was even attempted");
    assert_eq!(own(&view, cx), Some(("s1".to_owned(), 0)));
    assert_eq!(fleet.live("s1"), 1, "the other client's program runs on");
    // When that program ends too, the space is vacant and goes.
    fleet.kill_elsewhere("s1", &outside);
    until(&view, cx, "s1's server stops", 20, |_| !fleet.alive("s1"));
    assert_eq!(fleet.stops().len(), 1);
    assert!(fleet.alive("default"));
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_default_is_never_stopped(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let host = "keeps-default";
    let (view, cx) = connect(&mut fleet, host, "default", cx);
    close_the_tab(&view, cx);
    wait(&view, cx, "no tabs", |live| {
        live.connected && live.status.contains("No tabs")
    });
    throughout(&view, cx, "the default space stays", 7, |_| {
        fleet.alive("default")
    });
    assert!(fleet.stops().is_empty(), "no stop was even attempted");
    assert_eq!(own(&view, cx), Some(("default".to_owned(), 0)));
    assert!(fleet.alive("s1"));
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_a_failed_stop_is_an_error_and_the_space_stays(cx: &mut TestAppContext) {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let host = "refused";
    let refuse: Stopper = Arc::new(|_, _| Err(crate::Error::Ssh("host unreachable".into())));
    let (view, cx) = connect_with(&mut fleet, host, "s1", refuse, cx);
    close_the_tab(&view, cx);
    until(&view, cx, "the error is shown", 15, |cx| {
        status(&view, cx).contains("Could not remove Herdr 2 space s1: ")
    });
    assert!(status(&view, cx).contains("host unreachable"));
    throughout(&view, cx, "the space stays listed and running", 3, |cx| {
        fleet.alive("s1") && own(&view, cx) == Some(("s1".to_owned(), 0))
    });
    view.update(cx, |this, _| {
        assert!(
            this.terminal
                .core
                .as_ref()
                .is_some_and(|c| c.removal().is_none())
        );
    });
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_stop_refuses_a_server_other_than_the_one_seen() {
    let mut fleet = Fleet::start();
    fleet.add("s1");
    let stop = fleet.stopper();
    let error = stop(
        "s1",
        &Expect::seen("00000000000000000000000000000000".into(), None),
    )
    .err()
    .unwrap_or_else(|| panic!("a wrong server id stopped the server"));
    assert!(error.to_string().contains("left running"), "{error}");
    assert!(fleet.alive("s1"));
    let id = fleet.core_id("s1");
    stop("s1", &Expect::seen(id, None)).unwrap_or_else(|e| panic!("stop: {e}"));
    assert!(!fleet.alive("s1"));
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn empty_space_found_by_polls_is_stopped_after_two_snapshots_five_seconds_apart() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1", "s2", "s4"] {
        fleet.add(space);
    }
    // A terminal in no tab of yebisu keeps s4.
    let _outside = fleet.run_elsewhere("s4");
    let lists: SessionList = Default::default();
    let host = "polled";
    let t0 = Instant::now();
    let first = fleet.discover();
    record_discovery_at(&lists, None, host, Some(&first), t0)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    assert!(
        due_vacancies(&lists, t0, "").is_empty(),
        "one snapshot never stops a space"
    );
    // Another client creates a space just now.
    fleet.add("s3");
    let t1 = t0 + Duration::from_secs(5);
    record_discovery_at(&lists, None, host, Some(&fleet.discover()), t1)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    let due = due_vacancies(&lists, t1, "");
    let names = due
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    // s1 and s2 were vacant 5 s ago and now; default is protected; s3 is
    // younger than the grace; s4 has a live terminal. The connected space's
    // own worker decides for itself.
    assert_eq!(names, ["s1", "s2"]);
    assert_eq!(
        due_vacancies(&lists, t1, "s1")
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["s2"]
    );
    let stop = fleet.stopper();
    for vacant in &due {
        retire_vacant(&lists, None, host, vacant, &stop).unwrap_or_else(|e| panic!("stop: {e}"));
    }
    assert!(!fleet.alive("s1") && !fleet.alive("s2"));
    for kept in ["default", "s3", "s4"] {
        assert!(fleet.alive(kept), "{kept} keeps running");
    }
    assert_eq!(fleet.stops().len(), 2, "only s1 and s2 were stopped");
    // A poll that began before the stop and still lists them does not bring
    // them back.
    record_discovery_at(&lists, None, host, Some(&first), t1)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    let listed = |lists: &SessionList| {
        lists
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .discovery
            .rows
            .iter()
            .flatten()
            .map(|row| row.name.clone())
            .collect::<Vec<_>>()
    };
    assert!(
        !listed(&lists)
            .iter()
            .any(|name| name == "s1" || name == "s2")
    );
    // The next poll is the real one again: s3 has been vacant since t1.
    record_discovery_at(&lists, None, host, Some(&fleet.discover()), t1)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    // The young space goes once it has stayed vacant for the grace too.
    let t2 = t1 + Duration::from_secs(5);
    record_discovery_at(&lists, None, host, Some(&fleet.discover()), t2)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    let due = due_vacancies(&lists, t2, "");
    assert_eq!(
        due.iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["s3"]
    );
    retire_vacant(&lists, None, host, &due[0], &stop).unwrap_or_else(|e| panic!("stop: {e}"));
    assert!(!fleet.alive("s3"));
    assert!(fleet.alive("s4") && fleet.alive("default"));
}
