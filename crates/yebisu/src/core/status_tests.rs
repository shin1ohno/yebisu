//! The Herdr 2 status poll (review e85e SF1, SF2): a list that never
//! returns holds neither input nor Resize, the poll stops and is joined when
//! its guard drops, and the join has a bound.
use super::*;
use crate::{
    core::{Command, Session, screen::probe},
    shell::single_pane::Mailbox,
};
use sheprd_backend::{ClientPaneInputEvent, Size};
use std::{
    sync::atomic::AtomicUsize,
    time::{Duration, Instant},
};

/// Generous for a loaded shared host; a blocked list waits far longer.
const BOUND: Duration = Duration::from_secs(10);

fn raw(address: &str) -> CoreStatus {
    CoreStatus {
        address: address.into(),
        status: None,
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + BOUND;
    while !done() {
        assert!(Instant::now() < deadline, "{what} not within {BOUND:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A listing that blocks until `release` is dropped, reporting each entry.
fn blocked_list() -> (List, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered_tx, entered) = mpsc::channel();
    let (release, wait) = mpsc::channel::<()>();
    let list: List = Box::new(move || {
        let _ = entered_tx.send(());
        let _ = wait.recv();
        None
    });
    (list, entered, release)
}

fn start(list: List, stop: Arc<AtomicBool>) -> StatusPoll {
    StatusPoll::start(
        list,
        StatusMailbox::default(),
        Arc::new(Mutex::new(Live::default())),
        stop,
        Duration::from_millis(1),
    )
    .unwrap_or_else(|e| panic!("poll: {e}"))
}

#[test]
fn a_list_that_never_returns_does_not_delay_input_or_resize() {
    let (list, entered, release) = blocked_list();
    let (backend, wire) = crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let inbox = Arc::new(Mutex::new(Live::default()));
    let generation = 41;
    let session = Session::start_with(
        "status-poll-blocked".into(),
        generation,
        inbox.clone(),
        move || Ok(backend),
        None,
        Mailbox::detached(),
        PollPlan {
            source: StatusSource::Custom(list),
            period: Duration::from_millis(1),
        },
    )
    .unwrap_or_else(|e| panic!("start: {e}"));
    entered
        .recv_timeout(BOUND)
        .unwrap_or_else(|e| panic!("the poll never listed: {e}"));
    let mut destination = None;
    wait_until("the first core frame", || {
        let live = inbox.lock().unwrap_or_else(PoisonError::into_inner).clone();
        destination = crate::terminal_input::destination(&live, generation);
        destination.is_some()
    });
    let destination = destination.unwrap_or_else(|| panic!("destination"));

    // One input reaches the core while the list is still blocked.
    let sent = Instant::now();
    session
        .send(Command::Input(
            destination,
            ClientPaneInputEvent::Paste("status-poll-input".into()),
        ))
        .unwrap_or_else(|e| panic!("input: {e}"));
    let delivered = loop {
        let left = BOUND.saturating_sub(sent.elapsed());
        let (_, bytes) = wire
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("input was not delivered within {BOUND:?}"));
        if bytes
            .windows(b"status-poll-input".len())
            .any(|w| w == b"status-poll-input")
        {
            break sent.elapsed();
        }
    };
    assert!(delivered < BOUND, "input took {delivered:?}");

    // One Resize reaches the worker while the list is still blocked.
    let whole = Size::new(97, 31).unwrap_or_else(|e| panic!("size: {e}"));
    session
        .send(Command::Resize(whole))
        .unwrap_or_else(|e| panic!("resize: {e}"));
    wait_until("the Resize", || {
        (1..=2).any(|count| !probe::calls_for(whole, count).is_empty())
    });
    assert!(
        entered.try_recv().is_err(),
        "fixture: the first list never returned while input and Resize went through"
    );
    drop(session);
    drop(release);
}

#[test]
fn the_poll_stops_and_is_joined_when_its_guard_drops() {
    let held = Arc::new(());
    let listing = held.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let list: List = Box::new(move || {
        let _ = Arc::strong_count(&listing);
        counted.fetch_add(1, Ordering::AcqRel);
        Some(vec![raw("t1")])
    });
    let poll = start(list, Arc::new(AtomicBool::new(false)));
    wait_until("three lists", || calls.load(Ordering::Acquire) >= 3);
    let dropped = Instant::now();
    drop(poll);
    let took = dropped.elapsed();
    assert_eq!(
        Arc::strong_count(&held),
        1,
        "the poll thread ended (its listing dropped) before the guard's drop returned"
    );
    assert!(took < JOIN_BOUND, "an idle poll stops at once: {took:?}");
    let after = calls.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        calls.load(Ordering::Acquire),
        after,
        "no list after the drop"
    );
}

#[test]
fn the_session_stop_ends_the_poll() {
    let held = Arc::new(());
    let listing = held.clone();
    let list: List = Box::new(move || {
        let _ = Arc::strong_count(&listing);
        None
    });
    let stop = Arc::new(AtomicBool::new(false));
    let _poll = start(list, stop.clone());
    stop.store(true, Ordering::Release);
    wait_until("the poll thread's end", || Arc::strong_count(&held) == 1);
}

#[test]
fn dropping_the_guard_during_a_list_that_never_returns_is_bounded() {
    let (list, entered, release) = blocked_list();
    let poll = start(list, Arc::new(AtomicBool::new(false)));
    entered
        .recv_timeout(BOUND)
        .unwrap_or_else(|e| panic!("the poll never listed: {e}"));
    let (done, took) = mpsc::channel();
    std::thread::spawn(move || {
        let dropped = Instant::now();
        drop(poll);
        let _ = done.send(dropped.elapsed());
    });
    let took = took
        .recv_timeout(JOIN_BOUND + BOUND)
        .unwrap_or_else(|_| panic!("dropping the guard waited on the blocked list"));
    assert!(
        took < JOIN_BOUND + Duration::from_secs(2),
        "the join waited {took:?}"
    );
    drop(release);
}

#[test]
fn a_changed_list_marks_the_live_dirty_once() {
    let mailbox = StatusMailbox::default();
    let inbox = Arc::new(Mutex::new(Live::default()));
    let _poll = StatusPoll::start(
        Box::new(|| Some(vec![raw("t2")])),
        mailbox.clone(),
        inbox.clone(),
        Arc::new(AtomicBool::new(false)),
        Duration::from_millis(1),
    )
    .unwrap_or_else(|e| panic!("poll: {e}"));
    wait_until("the first list", || {
        mailbox.latest() == Some(vec![raw("t2")])
            && inbox.lock().unwrap_or_else(PoisonError::into_inner).dirty
    });
    inbox.lock().unwrap_or_else(PoisonError::into_inner).dirty = false;
    std::thread::sleep(Duration::from_millis(30));
    assert!(
        !inbox.lock().unwrap_or_else(PoisonError::into_inner).dirty,
        "the same list does not draw again"
    );
}

#[test]
fn the_core_listing_fills_the_session_statuses() {
    let (backend, _wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let session = Session::start_with(
        "status-poll-core".into(),
        42,
        Arc::new(Mutex::new(Live::default())),
        move || Ok(backend),
        None,
        Mailbox::detached(),
        PollPlan {
            source: StatusSource::Core,
            period: Duration::from_millis(10),
        },
    )
    .unwrap_or_else(|e| panic!("start: {e}"));
    let mut listed = None;
    wait_until("the core's terminal.list", || {
        listed = session.statuses();
        listed.as_ref().is_some_and(|l| !l.is_empty())
    });
    let listed = listed.unwrap_or_default();
    assert!(
        listed.iter().any(|s| s.address == "t1") && listed.iter().all(|s| s.status.is_none()),
        "the mock's terminals, none reporting a status: {listed:?}"
    );
}

#[test]
fn test_sessions_start_without_a_poll() {
    let (backend, _wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("peer: {e}"));
    let inbox = Arc::new(Mutex::new(Live::default()));
    let session = Session::start(
        "status-poll-off".into(),
        43,
        inbox.clone(),
        move || Ok(backend),
        None,
        Mailbox::detached(),
    )
    .unwrap_or_else(|e| panic!("start: {e}"));
    wait_until("the first core frame", || {
        inbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connected
    });
    std::thread::sleep(Duration::from_millis(50));
    assert!(session.statuses().is_none(), "no list in test builds");
}
