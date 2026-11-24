//! A Herdr 2 space listed only from its saved layout ("Session unconfirmed")
//! goes when a successful discovery of its host finds no server for it, and
//! its layout file with it; a failed discovery deletes nothing. Against real
//! herdr-core servers in a private fleet and a private store directory (opt in
//! like `empty_space_tests`).
use super::{
    Command, Session, SessionList, SessionTarget, due_vacancies, record_discovery_at,
    retire_vacant,
    space_fleet::Fleet,
    store::{self, Saved, Tab, Workspace},
    vacate::set_test_stopper,
};
use crate::live::Live;
use sheprd_backend::TerminalId;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// A private store directory, removed with the test.
struct Store(PathBuf);

impl Store {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("sv{:x}{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("store dir: {e}"));
        Self(dir)
    }

    fn root(&self) -> Option<&std::path::Path> {
        Some(&self.0)
    }

    /// Saves a layout of one tab holding terminal `t9`, the way a session
    /// that was open once left it.
    fn save(&self, host: &str, space: &str) {
        let id = TerminalId::new("t9").unwrap_or_else(|e| panic!("id: {e}"));
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
        store::save_workspace(self.root(), host, space, &workspace)
            .unwrap_or_else(|e| panic!("save {space}: {e}"));
    }

    fn saved(&self, host: &str) -> Vec<String> {
        store::sessions(self.root(), host).unwrap_or_else(|e| panic!("sessions: {e}"))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rows(lists: &SessionList) -> Vec<(String, bool)> {
    lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .discovery
        .rows
        .iter()
        .flatten()
        .map(|row| (row.name.clone(), row.confirmed))
        .collect()
}

fn poll(lists: &SessionList, store: &Store, host: &str, output: Option<&[u8]>) {
    record_discovery_at(lists, store.root(), host, output, Instant::now())
        .unwrap_or_else(|e| panic!("discovery: {e}"));
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn saved_space_without_a_server_is_removed_after_a_successful_discovery() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let store = Store::new("a");
    let host = "saved-only";
    // `gone` has a layout and no server anywhere; `s1` has both; `default`
    // has a layout and a server.
    for space in ["gone", "s1", "default"] {
        store.save(host, space);
    }
    let lists: SessionList = Default::default();
    poll(&lists, &store, host, Some(&fleet.discover()));
    let shown = rows(&lists);
    assert!(
        !shown.iter().any(|(name, _)| name == "gone"),
        "the saved-only space leaves the sidebar: {shown:?}"
    );
    assert!(shown.contains(&("s1".to_owned(), true)));
    assert!(shown.contains(&("default".to_owned(), true)));
    assert_eq!(
        store.saved(host),
        ["default", "s1"],
        "its layout file is deleted, the others stay"
    );
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_failed_discovery_keeps_the_rows_and_the_saved_layouts() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let store = Store::new("b");
    let host = "failing";
    for space in ["gone", "s1"] {
        store.save(host, space);
    }
    let lists: SessionList = Default::default();
    // Transport errors and timeouts: no output at all, twice in a row.
    poll(&lists, &store, host, None);
    poll(&lists, &store, host, None);
    assert_eq!(store.saved(host), ["gone", "s1"], "nothing was deleted");
    // After a good poll and then two failures, every row is unconfirmed
    // ("Session unconfirmed") and stays, with its layout.
    let first = fleet.discover();
    poll(&lists, &store, host, Some(&first));
    assert_eq!(store.saved(host), ["s1"]);
    store.save(host, "gone");
    poll(&lists, &store, host, None);
    poll(&lists, &store, host, None);
    let shown = rows(&lists);
    assert!(
        shown.iter().all(|(_, confirmed)| !confirmed) && shown.len() == 2,
        "every row stays, unconfirmed: {shown:?}"
    );
    assert_eq!(store.saved(host), ["gone", "s1"]);
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn a_stopped_server_whose_state_remains_goes_after_two_successful_discoveries() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let store = Store::new("c");
    let host = "stopped-elsewhere";
    store.save(host, "s1");
    // Someone ends s1's server with `hc server stop`: its state directory
    // stays, so discovery lists it without an answer.
    fleet.stop_space("s1").unwrap_or_else(|e| panic!("{e}"));
    assert!(!fleet.alive("s1"));
    let listing = String::from_utf8_lossy(&fleet.discover()).into_owned();
    assert!(listing.contains(r#"["s1", null]"#), "{listing}");
    let lists: SessionList = Default::default();
    poll(&lists, &store, host, Some(&fleet.discover()));
    assert!(
        rows(&lists).contains(&("s1".to_owned(), false)),
        "one unanswered poll could be a slow server: it stays unconfirmed"
    );
    assert_eq!(store.saved(host), ["s1"]);
    poll(&lists, &store, host, Some(&fleet.discover()));
    assert!(!rows(&lists).iter().any(|(name, _)| name == "s1"));
    assert!(store.saved(host).is_empty());
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn the_saved_layout_is_deleted_when_a_poll_finds_the_space_empty_and_it_is_stopped() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let store = Store::new("d");
    let host = "poll-stop";
    store.save(host, "s1");
    store.save(host, "default");
    let lists: SessionList = Default::default();
    let t0 = Instant::now();
    record_discovery_at(&lists, store.root(), host, Some(&fleet.discover()), t0)
        .unwrap_or_else(|e| panic!("discovery: {e}"));
    let t1 = t0 + Duration::from_secs(5);
    record_discovery_at(&lists, store.root(), host, Some(&fleet.discover()), t1)
        .unwrap_or_else(|e| panic!("discovery: {e}"));
    let due = due_vacancies(&lists, t1, "");
    assert_eq!(due.len(), 1, "{due:?}");
    retire_vacant(&lists, store.root(), host, &due[0], &fleet.stopper())
        .unwrap_or_else(|e| panic!("stop: {e}"));
    assert!(!fleet.alive("s1"));
    assert_eq!(store.saved(host), ["default"], "s1's layout is deleted");
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn the_saved_layout_is_deleted_when_the_session_stops_its_own_empty_space() {
    let mut fleet = Fleet::start();
    for space in ["default", "s1"] {
        fleet.add(space);
    }
    let store = Store::new("e");
    let host = "own-stop";
    set_test_stopper(host, fleet.stopper());
    let backend = fleet.bridge("s1");
    let inbox = Arc::new(Mutex::new(Live::default()));
    let session = Session::start_named(
        SessionTarget {
            host: host.into(),
            name: "s1".into(),
            focus: None,
            expected_core_id: None,
        },
        1,
        inbox.clone(),
        move || Ok(backend),
        store.root().map(PathBuf::from),
        crate::shell::single_pane::Mailbox::detached(),
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("session: {e}"));
    let tab_id = |session: &Session| {
        let epoch = inbox.lock().map(|live| live.input_epoch).unwrap_or(0);
        session
            .sessions(epoch)
            .into_iter()
            .find(|row| row.name == "s1")
            .and_then(|row| row.tabs.first().map(|tab| tab.id.clone()))
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let tab = loop {
        if let Some(tab) = tab_id(&session) {
            break tab;
        }
        assert!(Instant::now() < deadline, "the session never listed a tab");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(store.saved(host), ["s1"], "its layout is saved while open");
    session
        .send(Command::CloseTab(tab))
        .unwrap_or_else(|e| panic!("close: {e}"));
    let deadline = Instant::now() + Duration::from_secs(15);
    while session.removal().is_none() {
        assert!(Instant::now() < deadline, "the space was not removed");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!fleet.alive("s1"));
    assert!(fleet.alive("default"));
    assert!(store.saved(host).is_empty(), "its layout file is deleted");
}
