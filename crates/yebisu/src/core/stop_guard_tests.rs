//! The stop's guards, against real herdr-core servers in a private fleet (opt
//! in like `empty_space_tests`): a terminal started between the snapshot and
//! the stop, a space already stopped, a server restarted with the same core id,
//! a per-socket timeout, a failed stop shown over screen refreshes, and a
//! stopped space that another client starts again.
use super::{
    Command, Session, SessionList, SessionTarget, due_vacancies,
    poll::{listed_rows, record_discovery_at, retire_vacant},
    space_fleet::Fleet,
    store::{self, Saved, Tab, Workspace},
    vacate::{Expect, Stopped, Stopper},
};
use crate::live::Live;
use sheprd_backend::{Size, TerminalId};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn snapshot(lists: &SessionList, fleet: &Fleet, host: &str, at: Instant) {
    record_discovery_at(lists, None, host, Some(&fleet.discover()), at)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
}

/// The vacant spaces of `host` two snapshots, 5 s apart, agree on.
fn due(lists: &SessionList, fleet: &Fleet, host: &str) -> Vec<(String, Expect)> {
    let t0 = Instant::now();
    snapshot(lists, fleet, host, t0);
    let t1 = t0 + Duration::from_secs(5);
    snapshot(lists, fleet, host, t1);
    due_vacancies(lists, t1, "")
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_terminal_started_between_the_snapshot_and_the_stop_is_not_stopped() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let lists: SessionList = Default::default();
    let due = due(&lists, &fleet, "late-terminal");
    assert_eq!(due.len(), 1, "s1 is vacant in both snapshots: {due:?}");
    // Another client starts a program in s1 after the second snapshot.
    let _program = fleet.run_elsewhere("s1");
    let stopped = retire_vacant(&lists, None, "late-terminal", &due[0], &fleet.stopper())
        .unwrap_or_else(|e| panic!("stop: {e}"));
    assert_eq!(stopped, Stopped::Busy, "the stop saw the live terminal");
    assert!(fleet.alive("s1"), "the server and its program run on");
    assert_eq!(fleet.live("s1"), 1);
    assert!(
        listed_rows(&lists).iter().any(|row| row.name == "s1"),
        "the row stays"
    );
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn stopping_an_already_stopped_space_starts_no_server() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let seen = Expect::seen(fleet.core_id("s1"), Some(fleet.pid("s1")));
    // Someone else stopped it first; its state directory stays, as it does.
    fleet.stop_space("s1").unwrap_or_else(|e| panic!("{e}"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while fleet.alive("s1") {
        assert!(Instant::now() < deadline, "s1 did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    let stopped = (fleet.stopper())("s1", &seen).unwrap_or_else(|e| panic!("stop: {e}"));
    assert_eq!(stopped, Stopped::Done, "nothing to stop is stopped");
    std::thread::sleep(Duration::from_millis(500));
    assert!(!fleet.alive("s1"), "no server was started");
    assert!(!fleet.socket("s1").exists(), "no socket was made");
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_restarted_server_with_the_same_core_id_is_not_stopped_by_an_old_observation() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let (id, pid) = (fleet.core_id("s1"), fleet.pid("s1"));
    fleet.restart("s1");
    assert_eq!(fleet.core_id("s1"), id, "a core id outlives a restart");
    assert_ne!(
        fleet.pid("s1"),
        pid,
        "the restarted server is another process"
    );
    let stop = fleet.stopper();
    let error = stop("s1", &Expect::seen(id.clone(), Some(pid)))
        .err()
        .unwrap_or_else(|| panic!("the old observation stopped the restarted server"));
    assert!(
        error.to_string().contains("not the server that was seen"),
        "{error}"
    );
    assert!(fleet.alive("s1"));
    // A connection that is older than the server saw another one.
    let old = Expect {
        core_id: id,
        pid: None,
        min_uptime_ms: Some(600_000),
    };
    assert!(stop("s1", &old).is_err());
    assert!(fleet.alive("s1"));
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_per_socket_timeout_deletes_nothing() {
    let mut fleet = Fleet::start();
    fleet.add("default");
    fleet.silent_socket("s9");
    let root = std::env::temp_dir().join(format!("sg{:x}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    let id = TerminalId::new("t9").unwrap_or_else(|e| panic!("{e}"));
    let workspace = Workspace {
        created: 1,
        tabs: vec![Tab {
            id: "t9".into(),
            label: "Shell".into(),
            layout: Saved {
                ids: vec![id],
                ..Saved::default()
            },
        }],
        active: 0,
    };
    store::save_workspace(Some(&root), "slow", "s9", &workspace).unwrap_or_else(|e| panic!("{e}"));
    let output = fleet.discover();
    let text = String::from_utf8_lossy(&output).into_owned();
    assert!(
        text.contains(r#"["s9", "unknown"]"#),
        "a timeout is not absent: {text}"
    );
    let lists: SessionList = Default::default();
    for _ in 0..3 {
        record_discovery_at(
            &lists,
            Some(&root),
            "slow",
            Some(&fleet.discover()),
            Instant::now(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(
        store::sessions(Some(&root), "slow").unwrap_or_default(),
        ["s9"],
        "its layout is kept"
    );
    assert!(
        listed_rows(&lists)
            .iter()
            .any(|row| row.name == "s9" && !row.confirmed),
        "its row stays, unconfirmed"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_stop_error_survives_a_screen_refresh_until_the_user_acts() {
    let mut fleet = Fleet::start();
    for space in ["default", "s2"] {
        fleet.add(space);
    }
    let host = "notice";
    let backend = fleet.bridge("s2");
    let inbox = Arc::new(Mutex::new(Live::default()));
    let session = Session::start_named(
        SessionTarget {
            host: host.into(),
            name: "s2".into(),
            focus: None,
            expected_core_id: None,
        },
        1,
        inbox.clone(),
        move || Ok(backend),
        None,
        crate::shell::single_pane::Mailbox::detached(),
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("session: {e}"));
    let status = || {
        inbox
            .lock()
            .map(|live| live.status.clone())
            .unwrap_or_default()
    };
    let wait = |what: &str, done: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < deadline, "{what}: {}", status());
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait("connected", &|| {
        inbox.lock().is_ok_and(|live| live.connected)
    });
    // A poll fails to stop another space, s3.
    let refuse: Stopper = Arc::new(|_, _| Err(crate::Error::Ssh("host unreachable".into())));
    let vacant = ("s3".to_owned(), Expect::seen("abc".into(), Some(1)));
    assert!(retire_vacant(&session.sessions, None, host, &vacant, &refuse).is_err());
    // The worker publishes screens again (a resize is no user action).
    for columns in [90, 100, 110] {
        let size = Size::new(columns, 30).unwrap_or_else(|e| panic!("{e}"));
        session
            .send(Command::Resize(size))
            .unwrap_or_else(|e| panic!("{e}"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            status().contains("Could not remove Herdr 2 space s3: "),
            "the error was overwritten: {}",
            status()
        );
    }
    // The user acts (a new tab): the notice has been seen.
    session
        .send(Command::NewTab)
        .unwrap_or_else(|e| panic!("{e}"));
    wait("the notice goes with the user's action", &|| {
        !status().contains("Could not remove")
    });
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_stopped_space_that_another_client_starts_again_comes_back() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let host = "again";
    let lists: SessionList = Default::default();
    let stale = fleet.discover();
    let due = due(&lists, &fleet, host);
    assert_eq!(due.len(), 1);
    retire_vacant(&lists, None, host, &due[0], &fleet.stopper())
        .unwrap_or_else(|e| panic!("stop: {e}"));
    assert!(!fleet.alive("s1"));
    let hidden = |lists: &SessionList| !listed_rows(lists).iter().any(|row| row.name == "s1");
    // Its state directory remains, so a poll still lists the name: hidden
    // while its server does not answer, and for a poll that began before.
    snapshot(&lists, &fleet, host, Instant::now());
    assert!(hidden(&lists));
    record_discovery_at(&lists, None, host, Some(&stale), Instant::now())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        hidden(&lists),
        "the old server's answer does not bring it back"
    );
    // Another client starts the space again, with a program in it.
    fleet.restart("s1");
    let _program = fleet.run_elsewhere("s1");
    snapshot(&lists, &fleet, host, Instant::now());
    let row = listed_rows(&lists)
        .into_iter()
        .find(|row| row.name == "s1")
        .unwrap_or_else(|| panic!("the restarted space did not come back"));
    assert!(row.confirmed);
    assert_eq!(row.tabs.len(), 1, "its program is a tab");
}
