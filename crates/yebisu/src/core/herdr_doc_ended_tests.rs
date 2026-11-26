//! The reader removes a `herdr-app` space whose every terminal ended, over
//! the mock core: after an exit, a kill, a terminal gone with a restart and
//! a relaunch's first connection; never on the first read, after a failed
//! list, from an unreadable document, or from a space with a live terminal.
use super::*;
use crate::{
    core::{DocRows, doc_tabs::DocTabs, store::Workspace},
    core_mock::Host,
};
use serde_json::{Value, json};

/// Generous for a loaded shared host; the waits below are far shorter.
const BOUND: Duration = Duration::from_secs(10);
/// The poll period, and so the grace, of these readers.
const PERIOD: Duration = Duration::from_millis(100);

fn block(terminal: &str) -> Value {
    json!({"type": "block", "id": format!("p{terminal}"), "content": {"terminal": terminal}})
}

fn space(id: &str, tabs: &[(&str, Value)]) -> Value {
    let tabs = tabs
        .iter()
        .map(|(tab, root)| {
            // A tab's root carries the tab's id (H-5, the `root-id` gate).
            let mut root = root.clone();
            root["id"] = json!(tab);
            json!({"id": tab, "name": "", "root": root})
        })
        .collect::<Vec<_>>();
    json!({"id": id, "name": "", "folder": "/w", "tabs": tabs})
}

/// `sp1` holds `t1` alone; `sp2` holds live `t2` beside `t3`.
fn layout() -> Value {
    json!({"spaces": [
        space("sp1", &[("tb1", block("t1"))]),
        space("sp2", &[("tb2", block("t2")), ("tb3", block("t3"))]),
    ], "future": {"kept": true}})
}

struct Running {
    host: Host,
    mailbox: DocMailbox,
    seen: u64,
    _reader: (DocSync, DocWriter),
}

fn start(host: &Host, period: Duration) -> Running {
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
        host: host.clone(),
        mailbox,
        seen: 0,
        _reader: reader,
    }
}

impl Running {
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

    fn ids(&self) -> Vec<String> {
        let (value, _) = self.host.doc().doc();
        value["spaces"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|space| space["id"].as_str().map(str::to_owned))
            .collect()
    }
}

fn spaces(snapshot: &DocSnapshot) -> Vec<String> {
    match &snapshot.read {
        Read::Readable(view) => view.spaces.iter().map(|s| s.id.clone()).collect(),
        _ => Vec::new(),
    }
}

fn host_with(live: &[&str]) -> Host {
    let host = Host::default();
    host.set_live(live);
    host.doc().set_silent(layout());
    host
}

/// `sp1` leaves the document, and everything else stays byte for byte.
fn removes_sp1(host: &Host) {
    let mut running = start(host, PERIOD);
    let removed = running
        .next(BOUND, |snapshot| {
            !spaces(snapshot).contains(&"sp1".to_owned())
        })
        .unwrap_or_else(|| panic!("sp1 stayed: {:?}", running.ids()));
    assert_eq!(spaces(&removed), ["sp2"]);
    assert_eq!(removed.stats.ended, 1);
    let mut expected = layout();
    if let Some(Value::Array(spaces)) = expected.get_mut("spaces") {
        spaces.remove(0);
    }
    assert_eq!(host.doc().doc().0, expected, "sp2 keeps both its tabs");
    assert_eq!(host.doc().sets().len(), 1, "one write");
    assert_eq!(removed.stats.notice, None, "no user change was refused");
}

#[test]
fn exited_terminal_removes_its_space() {
    let host = host_with(&["t2"]);
    host.end("t1", "exited");
    host.end("t3", "exited");
    removes_sp1(&host);
}

#[test]
fn killed_terminal_removes_its_space() {
    let host = host_with(&["t2"]);
    host.end("t1", "killed");
    removes_sp1(&host);
}

#[test]
fn terminal_gone_after_a_restart_removes_its_space() {
    // The core no longer lists t1 at all (the official app's `not_found`).
    removes_sp1(&host_with(&["t2"]));
}

#[test]
fn relaunch_removes_a_space_that_ended_while_closed() {
    let host = host_with(&["t1", "t2"]);
    {
        let mut first = start(&host, PERIOD);
        first
            .next(BOUND, |snapshot| snapshot.live.contains(&"t1".to_owned()))
            .unwrap_or_else(|| panic!("no first read"));
    }
    // Ended while no yebisu was connected; the next launch connects anew.
    host.end("t1", "exited");
    removes_sp1(&host);
}

#[test]
fn live_terminal_keeps_its_space() {
    // The negative control: every terminal is live.
    let host = host_with(&["t1", "t2", "t3"]);
    let mut running = start(&host, PERIOD);
    std::thread::sleep(PERIOD * 6);
    let last = running.next(BOUND, |_| true);
    assert!(
        host.doc().sets().is_empty(),
        "written: {:?}",
        host.doc().sets()
    );
    assert_eq!(running.ids(), ["sp1", "sp2"], "{last:?}");
}

#[test]
fn first_read_never_removes() {
    let host = host_with(&["t2"]);
    // A period far longer than the wait: only the first read happens.
    let mut running = start(&host, Duration::from_secs(60));
    running
        .next(BOUND, |snapshot| snapshot.version == 1)
        .unwrap_or_else(|| panic!("no first read"));
    std::thread::sleep(Duration::from_millis(500));
    assert!(host.doc().sets().is_empty(), "removed on the first read");
}

#[test]
fn failed_list_removes_nothing_until_lists_answer_again() {
    let host = host_with(&["t2"]);
    host.doc().fail_lists(usize::MAX);
    let mut running = start(&host, PERIOD);
    std::thread::sleep(PERIOD * 6);
    assert!(host.doc().sets().is_empty(), "removed after failed lists");
    assert_eq!(running.ids(), ["sp1", "sp2"]);
    // The control: the same layout goes once the list answers.
    host.doc().fail_lists(0);
    running
        .next(BOUND, |snapshot| spaces(snapshot) == ["sp2"])
        .unwrap_or_else(|| panic!("sp1 stayed after lists answered"));
}

#[test]
fn unreadable_document_is_never_written() {
    let host = Host::default();
    host.set_live(&[]);
    host.doc().set_silent(
        serde_json::from_str(include_str!(
            "../../fixtures/herdr-app/unreadable-root.json"
        ))
        .unwrap_or_else(|e| panic!("fixture: {e}")),
    );
    let _running = start(&host, PERIOD);
    std::thread::sleep(PERIOD * 6);
    assert!(host.doc().sets().is_empty());
}

#[test]
fn removed_space_leaves_the_sidebar() {
    let host = host_with(&["t2"]);
    let mut running = start(&host, PERIOD);
    let first = running
        .next(BOUND, |snapshot| snapshot.version == 1)
        .unwrap_or_else(|| panic!("no first read"));
    let rows = |snapshot: DocSnapshot| {
        let mut doc = DocTabs::default();
        let mut workspace = Workspace::default();
        doc.take(snapshot, &mut workspace);
        match doc.catalog(&workspace).map(|catalog| catalog.state) {
            Some(DocRows::Readable(spaces)) => spaces
                .iter()
                .map(|s| (s.id.clone(), s.tabs.len()))
                .collect::<Vec<_>>(),
            other => panic!("rows: {other:?}"),
        }
    };
    // Before: the ended space is a row with its tab count, as the owner saw.
    assert_eq!(rows(first), [("sp1".to_owned(), 1), ("sp2".to_owned(), 2)]);
    let after = running
        .next(BOUND, |snapshot| spaces(snapshot) == ["sp2"])
        .unwrap_or_else(|| panic!("sp1 stayed"));
    assert_eq!(rows(after), [("sp2".to_owned(), 2)]);
}
