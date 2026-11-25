//! The `herdr-app` reader (#144 D2, AC1 and AC3): the first read, a doc
//! event or `missed` reads again, a change without an event is seen within
//! the poll period, and test builds read nothing unless asked.
use super::*;
use crate::core_mock::Host;
use serde_json::{Value, json};

/// Generous for a loaded shared host; the waits below are far shorter.
const BOUND: Duration = Duration::from_secs(10);

fn two_spaces() -> Value {
    serde_json::from_str(include_str!("../../fixtures/herdr-app/two-spaces.json"))
        .unwrap_or_else(|e| panic!("fixture: {e}"))
}

struct Running {
    host: Host,
    mailbox: DocMailbox,
    seen: u64,
    _reader: DocSync,
}

fn start(host: Host, period: Duration) -> Running {
    let (backend, _) = host
        .recorded(false)
        .unwrap_or_else(|e| panic!("mock core: {e}"));
    let mailbox = DocMailbox::default();
    let reader = DocSync::start(
        DocPlan {
            source: DocSource::Core,
            period,
        },
        (&backend, "pro-dev"),
        mailbox.clone(),
        Arc::new(Mutex::new(Live::default())),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_or_else(|e| panic!("reader: {e}"))
    .unwrap_or_else(|| panic!("a Core plan reads"));
    Running {
        host,
        mailbox,
        seen: 0,
        _reader: reader,
    }
}

impl Running {
    /// The next snapshot matching `want`, within `within`.
    fn next(
        &mut self,
        within: Duration,
        want: impl Fn(&DocSnapshot) -> bool,
    ) -> Option<DocSnapshot> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Some(snapshot) = self.mailbox.since(&mut self.seen)
                && want(&snapshot)
            {
                return Some(snapshot);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }
}

fn version(want: u64) -> impl Fn(&DocSnapshot) -> bool {
    move |snapshot| snapshot.version == want
}

#[test]
fn initial_read_publishes_snapshot() {
    let host = Host::default();
    host.doc().set_silent(two_spaces());
    let mut running = start(host, Duration::from_secs(60));
    let snapshot = running
        .next(BOUND, version(1))
        .unwrap_or_else(|| panic!("no first read"));
    let Read::Readable(view) = &snapshot.read else {
        panic!("not readable: {:?}", snapshot.read);
    };
    let names = view
        .spaces
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Build", ""]);
    assert_eq!(snapshot.live, ["t1", "t2"]);
    assert!(snapshot.names.is_empty(), "no host is asked in tests");
}

#[test]
fn null_document_reads_as_empty() {
    let mut running = start(Host::default(), Duration::from_secs(60));
    let snapshot = running
        .next(BOUND, version(0))
        .unwrap_or_else(|| panic!("no first read"));
    assert_eq!(snapshot.read, Read::Readable(View::default()));
}

#[test]
fn unreadable_document_is_reported() {
    let host = Host::default();
    host.doc().set_silent(
        serde_json::from_str(include_str!(
            "../../fixtures/herdr-app/unreadable-root.json"
        ))
        .unwrap_or_else(|e| panic!("fixture: {e}")),
    );
    let mut running = start(host, Duration::from_secs(60));
    let snapshot = running
        .next(BOUND, version(1))
        .unwrap_or_else(|| panic!("no first read"));
    assert_eq!(snapshot.read, Read::Unreadable);
}

#[test]
fn doc_event_triggers_reread() {
    // The poll period is far longer than the wait: only the event can do it.
    let mut running = start(Host::default(), Duration::from_secs(60));
    running
        .next(BOUND, version(0))
        .unwrap_or_else(|| panic!("first"));
    let (writer, _) = running
        .host
        .recorded(false)
        .unwrap_or_else(|e| panic!("writer: {e}"));
    writer
        .core()
        .doc_set(herdr_doc::NAME, &two_spaces(), 0)
        .unwrap_or_else(|e| panic!("write: {e}"));
    let snapshot = running.next(Duration::from_secs(5), version(1));
    assert!(snapshot.is_some(), "the doc event did not reread");
}

#[test]
fn missed_triggers_reread() {
    let mut running = start(Host::default(), Duration::from_secs(60));
    running
        .next(BOUND, version(0))
        .unwrap_or_else(|| panic!("first"));
    // Negative control: a change without an event is not read yet.
    running.host.doc().set_silent(json!({"spaces": []}));
    assert!(
        running
            .next(Duration::from_millis(600), version(1))
            .is_none()
    );
    running.host.doc().missed();
    assert!(running.next(Duration::from_secs(5), version(1)).is_some());
}

#[test]
fn silent_change_is_seen_within_poll() {
    let mut running = start(Host::default(), Duration::from_millis(200));
    running
        .next(BOUND, version(0))
        .unwrap_or_else(|| panic!("first"));
    running.host.doc().set_silent(json!({"spaces": []}));
    assert!(running.next(Duration::from_secs(2), version(1)).is_some());
}

#[test]
fn doc_poll_is_at_most_five_seconds() {
    // AC3: without events the document is followed within five seconds.
    assert!(DOC_POLL <= Duration::from_secs(5));
    assert!(EVENT_WAIT < DOC_POLL);
}

#[test]
fn test_builds_read_nothing_unless_asked() {
    // K6: transcript-exact tests (publication_probe) see no doc requests.
    assert!(matches!(DocPlan::default().source, DocSource::Off));
    let (backend, _) = Host::default()
        .recorded(false)
        .unwrap_or_else(|e| panic!("mock core: {e}"));
    let reader = DocSync::start(
        DocPlan::default(),
        (&backend, "pro-dev"),
        DocMailbox::default(),
        Arc::new(Mutex::new(Live::default())),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_or_else(|e| panic!("reader: {e}"));
    assert!(reader.is_none());
}

#[test]
fn mailbox_hands_each_change_once() {
    let mailbox = DocMailbox::default();
    let snapshot = DocSnapshot {
        version: 1,
        read: Read::Unreadable,
        live: Vec::new(),
        names: BTreeMap::new(),
    };
    let mut seen = 0;
    assert!(mailbox.since(&mut seen).is_none());
    assert!(mailbox.publish(snapshot.clone()));
    assert!(
        !mailbox.publish(snapshot.clone()),
        "the same read is no change"
    );
    assert_eq!(mailbox.since(&mut seen), Some(snapshot));
    assert!(mailbox.since(&mut seen).is_none());
}

#[test]
fn dropping_the_reader_ends_it_within_the_bound() {
    let running = start(Host::default(), Duration::from_millis(200));
    let started = Instant::now();
    drop(running);
    assert!(started.elapsed() < BOUND);
}
