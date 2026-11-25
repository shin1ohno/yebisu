//! The `herdr-app` write path (#144 D4, AC2 and AC6-AC10): the version
//! check, applying the intent again to a newer document at most three times,
//! never the previous value under a newer version, nothing sent when a gate
//! or an unreadable document refuses it, one intent at a time, and the
//! worker's New tab, split, close and New space in a document space.
use super::*;
use crate::{
    core::{Command, Session},
    core_mock::Host,
};
use serde_json::{Value, json};

/// Generous for a loaded shared host; the waits below are far shorter.
const BOUND: Duration = Duration::from_secs(10);
/// The two spaces of `two-spaces.json`, and the first one's first tab.
const B1: &str = "sp00000000000000000000000000000015";
const TB15: &str = "tb00000000000000000000000000000015";

fn fixture(name: &str) -> Value {
    let text = match name {
        "two-spaces" => include_str!("../../fixtures/herdr-app/two-spaces.json"),
        "tree-mod" => include_str!("../../fixtures/herdr-app/tree-mod.json"),
        "unreadable-root" => include_str!("../../fixtures/herdr-app/unreadable-root.json"),
        _ => panic!("no fixture {name}"),
    };
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + BOUND;
    while !done() {
        assert!(Instant::now() < deadline, "{what} not within {BOUND:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A reader on its own (no worker): intents go straight to its queue.
struct Writing {
    host: Host,
    mailbox: DocMailbox,
    writer: DocWriter,
    _reader: DocSync,
}

fn writing(host: Host, name: &str) -> Writing {
    let (backend, _) = host
        .recorded(false)
        .unwrap_or_else(|e| panic!("mock core: {e}"));
    let mailbox = DocMailbox::default();
    let (reader, writer) = DocSync::start(
        DocPlan {
            source: DocSource::Core,
            period: Duration::from_secs(60),
        },
        (&backend, name),
        mailbox.clone(),
        Arc::new(Mutex::new(Live::default())),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_or_else(|e| panic!("reader: {e}"))
    .unwrap_or_else(|| panic!("a Core plan reads"));
    Writing {
        host,
        mailbox,
        writer,
        _reader: reader,
    }
}

impl Writing {
    fn latest(&self) -> Option<DocSnapshot> {
        let mut seen = 0;
        self.mailbox.since(&mut seen)
    }

    /// Waits for the first read, sends `intents`, and returns the stats once
    /// they all finished.
    fn run(&self, intents: Vec<Intent>) -> WriteStats {
        wait_until("the first read", || self.latest().is_some());
        let count = intents.len() as u64;
        for intent in intents {
            self.writer
                .send(intent)
                .unwrap_or_else(|e| panic!("queue: {e}"));
        }
        wait_until("every intent finished", || {
            self.latest().is_some_and(|s| s.stats.finished == count)
        });
        self.latest()
            .map(|snapshot| snapshot.stats)
            .unwrap_or_default()
    }
}

fn rename(name: &str) -> Intent {
    Intent::RenameSpace {
        id: B1.into(),
        name: name.into(),
    }
}

fn seeded(name: &str) -> Host {
    let host = Host::default();
    host.doc().set_silent(fixture(name));
    host
}

#[test]
fn sends_original_version() {
    let writing = writing(seeded("two-spaces"), "pro-dev");
    let stats = writing.run(vec![rename("Alpha")]);
    assert_eq!(stats.last.as_deref(), Some("saved"));
    let sets = writing.host.doc().sets();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].1, 1, "the version the document was read at");
    assert_eq!(sets[0].0["spaces"][0]["name"], json!("Alpha"));
    // R1: the written document is read back.
    let latest = writing.latest().unwrap_or_else(|| panic!("snapshot"));
    assert_eq!(latest.version, 2);
}

#[test]
fn conflict_reapplies_intent_to_new_document() {
    let host = seeded("two-spaces");
    host.doc().conflict_next(1);
    let writing = writing(host, "pro-dev");
    let stats = writing.run(vec![rename("Alpha")]);
    assert_eq!((stats.last.as_deref(), stats.conflicts), (Some("saved"), 1));
    let sets = writing.host.doc().sets();
    assert_eq!(sets.len(), 2);
    assert_eq!(
        (sets[0].1, sets[1].1),
        (1, 2),
        "the second over the newer version"
    );
    // Applied to the newer document: both changes are in what was sent.
    assert_eq!(sets[1].0["external"], json!(1));
    assert_eq!(sets[1].0["spaces"][0]["name"], json!("Alpha"));
    assert_ne!(sets[0].0, sets[1].0);
    let (stored, _) = writing.host.doc().doc();
    assert_eq!(stored, sets[1].0);
}

#[test]
fn never_resends_previous_value() {
    // The document gets a newer version with the same value, unseen by the
    // reader: the conflict's read gives the value already sent once.
    let host = seeded("two-spaces");
    let writing = writing(host, "pro-dev");
    wait_until("the first read", || writing.latest().is_some());
    let (value, _) = writing.host.doc().doc();
    writing.host.doc().set_silent(value);
    let stats = writing.run(vec![rename("Alpha")]);
    assert_eq!(stats.last.as_deref(), Some("conflict-lost"));
    assert_eq!(stats.notice.as_deref(), Some(CHANGED_ELSEWHERE));
    let sets = writing.host.doc().sets();
    assert_eq!(sets.len(), 1, "the same value is not sent again: {sets:?}");
}

#[test]
fn three_conflicts_drop_and_notify() {
    let host = seeded("two-spaces");
    host.doc().conflict_next(5);
    let writing = writing(host, "pro-dev");
    let stats = writing.run(vec![rename("Alpha")]);
    assert_eq!(stats.last.as_deref(), Some("conflict-lost"));
    assert_eq!(stats.notice.as_deref(), Some(CHANGED_ELSEWHERE));
    let sets = writing.host.doc().sets();
    // Owner decision 5: three sends, not whatever the constant says.
    assert_eq!(sets.len(), 3, "at most three sends");
    for pair in sets.windows(2) {
        assert_ne!(pair[0].0, pair[1].0, "never the same value twice");
    }
    let (stored, _) = writing.host.doc().doc();
    assert_ne!(stored["spaces"][0]["name"], json!("Alpha"));
}

/// `intent` refused by a gate: nothing reaches the mock core.
fn refused(intent: Intent, gate: &str) {
    let writing = writing(seeded("two-spaces"), "pro-dev");
    let stats = writing.run(vec![intent]);
    assert_eq!(stats.last.as_deref(), Some(gate));
    assert!(stats.notice.is_some());
    assert!(
        writing.host.doc().sets().is_empty(),
        "{gate}: a doc.set was sent"
    );
}

#[test]
fn check_failure_sends_nothing_size() {
    let mut big = fixture("two-spaces");
    big["spaces"][0]["x"] = json!("x".repeat(herdr_doc::MAX_BYTES));
    refused(Intent::Replace(big), "check-failed:too-large");
}

#[test]
fn check_failure_sends_nothing_shape() {
    refused(
        Intent::Replace(json!({"spaces": {}})),
        "check-failed:unreadable",
    );
}

#[test]
fn check_failure_sends_nothing_path() {
    refused(
        Intent::RenameSpaceAlsoTouch {
            id: B1.into(),
            name: "Alpha".into(),
        },
        "check-failed:outside-path",
    );
}

#[test]
fn check_failure_sends_nothing_ids() {
    let mut root = fixture("two-spaces");
    root["spaces"][0]["tabs"][0]["root"]["id"] = json!("tbother");
    refused(Intent::Replace(root), "check-failed:root-id");
    let mut twin = fixture("two-spaces");
    let block = twin["spaces"][0]["tabs"][0]["root"]["children"][0].clone();
    if let Some(children) = twin["spaces"][0]["tabs"][0]["root"]["children"].as_array_mut() {
        let mut copy = block;
        copy["content"] = json!({"terminal": "t9"});
        children.push(copy);
    }
    refused(Intent::Replace(twin), "check-failed:duplicate-pane");
}

#[test]
fn unreadable_document_sends_no_doc_set() {
    let writing = writing(seeded("unreadable-root"), "pro-dev");
    let stats = writing.run(vec![
        rename("Alpha"),
        Intent::NewSpace {
            space_id: "spnew".into(),
            folder: "/tmp".into(),
            tab_id: "tbnew".into(),
            pane_id: "pnew".into(),
            terminal: "t9".into(),
        },
    ]);
    assert_eq!(stats.last.as_deref(), Some("not-readable"));
    assert!(writing.host.doc().sets().is_empty());
}

#[test]
fn unchanged_intent_sends_nothing() {
    let writing = writing(seeded("two-spaces"), "pro-dev");
    let stats = writing.run(vec![rename("Build")]);
    assert_eq!(stats.last.as_deref(), Some("unchanged"));
    assert!(writing.host.doc().sets().is_empty());
}

#[test]
fn second_intent_waits_for_first_result() {
    // D-6: the second is applied to the document the first produced.
    let writing = writing(seeded("two-spaces"), "pro-dev");
    let second = Intent::RenameTab {
        id: TB15.into(),
        name: "first".into(),
    };
    let stats = writing.run(vec![rename("Alpha"), second]);
    assert_eq!(stats.writes, 2);
    let sets = writing.host.doc().sets();
    assert_eq!((sets[0].1, sets[1].1), (1, 2));
    assert_eq!(sets[1].0["spaces"][0]["name"], json!("Alpha"));
}

#[test]
fn an_unsent_intent_is_applied_once_on_the_next_connection() {
    // D-7: the outcome of a write the connection lost is unknown; the next
    // reader of the host applies it once more, and an intent that landed is
    // `Unchanged` then.
    let host = seeded("two-spaces");
    unsent().insert("pro-dev-d7".into(), rename("Alpha"));
    let first = writing(host, "pro-dev-d7");
    wait_until("the unsent intent", || {
        first.latest().is_some_and(|s| s.stats.finished == 1)
    });
    assert_eq!(first.host.doc().sets().len(), 1);
    assert!(!unsent().contains_key("pro-dev-d7"));
    drop(first);
    let host = Host::default();
    host.doc().set_silent(fixture("two-spaces"));
    let landed = host.doc().doc().0;
    let mut renamed = landed;
    renamed["spaces"][0]["name"] = json!("Alpha");
    host.doc().set_silent(renamed);
    unsent().insert("pro-dev-d7b".into(), rename("Alpha"));
    let again = writing(host, "pro-dev-d7b");
    wait_until("the second reader's intent", || {
        again.latest().is_some_and(|s| s.stats.finished == 1)
    });
    let stats = again.latest().map(|s| s.stats).unwrap_or_default();
    assert_eq!(stats.last.as_deref(), Some("unchanged"));
    assert!(again.host.doc().sets().is_empty());
}

// The worker in a document space (#144 D4).

struct Worker {
    host: Host,
    session: Session,
    inbox: Arc<Mutex<Live>>,
}

fn worker(host: Host) -> Worker {
    let (backend, _) = host
        .recorded(false)
        .unwrap_or_else(|e| panic!("mock core: {e}"));
    let inbox = Arc::new(Mutex::new(Live::default()));
    let session = Session::start_doc(
        "pro-dev".into(),
        1,
        inbox.clone(),
        move || Ok(backend),
        None,
        crate::shell::single_pane::Mailbox::detached(),
        DocPlan {
            source: DocSource::Core,
            period: Duration::from_millis(200),
        },
    )
    .unwrap_or_else(|e| panic!("start: {e}"));
    let worker = Worker {
        host,
        session,
        inbox,
    };
    wait_until("the worker drew", || worker.live().connected);
    worker
}

impl Worker {
    fn live(&self) -> Live {
        self.inbox
            .lock()
            .map(|live| live.clone())
            .unwrap_or_default()
    }

    fn send(&self, command: Command) {
        self.session
            .send(command)
            .unwrap_or_else(|e| panic!("send: {e}"));
    }

    fn space_tabs(&self, space: usize) -> Vec<Value> {
        let (value, _) = self.host.doc().doc();
        value["spaces"][space]["tabs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn labels_of(&self, terminal: &Value) -> Value {
        self.host
            .doc()
            .labels(terminal.as_str().unwrap_or_default())
    }
}

#[test]
fn new_tab_runs_terminal_with_labels() {
    let worker = worker(seeded("two-spaces"));
    worker.send(Command::NewTab);
    wait_until("the new tab in the document", || {
        worker.space_tabs(0).len() == 3
    });
    let tab = &worker.space_tabs(0)[2];
    let terminal = &tab["root"]["children"][0]["content"]["terminal"];
    assert_eq!(
        worker.labels_of(terminal),
        json!({"herdr.seen": "0", "herdr.space": B1, "herdr.tab": tab["id"]})
    );
    assert_eq!(tab["root"]["id"], tab["id"]);
    assert_eq!(worker.host.doc().sets().len(), 1);
}

#[test]
fn split_spawns_with_labels() {
    let worker = worker(seeded("two-spaces"));
    worker.send(Command::SplitInDirection(false));
    wait_until("the split in the document", || {
        worker.space_tabs(0)[0]["root"]["children"]
            .as_array()
            .is_some_and(|children| children.len() == 2)
    });
    let added = &worker.space_tabs(0)[0]["root"]["children"][1];
    assert_eq!(added["type"], json!("block"));
    let terminal = &added["content"]["terminal"];
    assert_eq!(
        worker.labels_of(terminal),
        json!({"herdr.seen": "0", "herdr.space": B1, "herdr.tab": TB15})
    );
}

#[test]
fn close_kills_without_doc_set() {
    // Decision 7, AC9: the terminal ends; the tree keeps its block.
    let worker = worker(seeded("two-spaces"));
    let (before, _) = worker.host.doc().doc();
    worker.send(Command::CloseTab(TB15.into()));
    let (probe, _) = worker
        .host
        .recorded(false)
        .unwrap_or_else(|e| panic!("probe: {e}"));
    wait_until("t1 ended", || {
        probe
            .core()
            .list(false)
            .is_ok_and(|list| list.terminals.iter().all(|t| t.address != "t1"))
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(worker.host.doc().sets().is_empty());
    assert_eq!(worker.host.doc().doc().0, before);
}

#[test]
fn new_doc_space_appends_space() {
    let worker = worker(seeded("two-spaces"));
    worker.send(Command::NewDocSpace);
    let spaces = |worker: &Worker| {
        let (value, _) = worker.host.doc().doc();
        value["spaces"].as_array().cloned().unwrap_or_default()
    };
    wait_until("the new space", || spaces(&worker).len() == 3);
    let space = &spaces(&worker)[2];
    assert_eq!(space["name"], json!(""));
    assert_eq!(space["folder"], json!("/tmp"), "the new terminal's folder");
    let terminal = &space["tabs"][0]["root"]["children"][0]["content"]["terminal"];
    assert_eq!(worker.labels_of(terminal)["herdr.space"], space["id"]);
}

#[test]
fn new_space_and_split_are_refused_where_nothing_can_be_written() {
    // Q6: no New space while the document is unreadable.
    let worker = worker(seeded("unreadable-root"));
    worker.send(Command::NewDocSpace);
    wait_until("the refusal", || {
        worker
            .live()
            .status
            .contains("Can't read the herdr-app layout; New space needs it")
    });
    assert!(worker.host.doc().sets().is_empty());
    // A tab whose tree yebisu cannot draw is not split (Design F).
    let worker = self::worker(seeded("tree-mod"));
    worker.send(Command::SplitInDirection(false));
    wait_until("the split refusal", || {
        worker.live().status.contains("can't be split on the iPad")
    });
    assert!(worker.host.doc().sets().is_empty());
}
