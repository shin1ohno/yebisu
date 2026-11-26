//! Which `herdr-app` terminals count as ended, which spaces that changes,
//! and when a change is due: each way a terminal ends (exit, kill, a server
//! stop, gone after a restart or a relaunch), each thing that keeps a pane
//! (a live terminal, a kept terminal, another machine's terminal, a mod),
//! and each guard (the first read, the grace, another boot, a failed read,
//! a changed space).
use super::ended_spaces::{Candidate, Listing, Tracker, ended, unlisted};
use super::herdr_doc::{Intent, parse_view};
use serde_json::{Value, json};
use sheprd_core_client::wire::Terminal;
use std::time::{Duration, Instant};

const GRACE: Duration = Duration::from_secs(5);

fn block(terminal: &str) -> Value {
    json!({"type": "block", "id": format!("p-{terminal}"), "content": {"terminal": terminal}})
}

fn tab(id: &str, root: Value) -> Value {
    json!({"id": id, "name": "", "root": root})
}

fn space(id: &str, tabs: Vec<Value>) -> Value {
    json!({"id": id, "name": "", "folder": "/w", "tabs": tabs})
}

fn doc(spaces: Vec<Value>) -> Value {
    json!({"spaces": spaces, "future": {"kept": true}})
}

fn listing(terminals: &[(&str, &str, Option<&str>)]) -> Listing {
    let terminals = terminals
        .iter()
        .map(|(address, state, reason)| {
            serde_json::from_value::<Terminal>(
                json!({"address": address, "state": state, "reason": reason}),
            )
            .unwrap_or_else(|e| panic!("terminal: {e}"))
        })
        .collect::<Vec<_>>();
    Listing::new(&terminals)
}

fn ended_ids(value: &Value, listing: &Listing) -> Vec<String> {
    let view = parse_view(value).unwrap_or_else(|e| panic!("view: {e}"));
    ended(value, &view, listing)
        .into_iter()
        .map(|candidate| candidate.id)
        .collect()
}

fn one(terminal: &str) -> Value {
    doc(vec![space("sp1", vec![tab("tb1", block(terminal))])])
}

#[test]
fn exited_terminal_ends_its_space() {
    let found = ended_ids(&one("t1"), &listing(&[("t1", "finished", Some("exited"))]));
    assert_eq!(found, ["sp1"]);
}

#[test]
fn killed_terminal_ends_its_space() {
    let found = ended_ids(&one("t1"), &listing(&[("t1", "finished", Some("killed"))]));
    assert_eq!(found, ["sp1"]);
}

#[test]
fn terminal_core_no_longer_knows_ends_its_space() {
    // After a restart that dropped it, or one removed while the app was
    // closed: the official app's `not_found`.
    let value = one("t1");
    let mut listed = listing(&[("t9", "live", None)]);
    // Left out of the list alone ends nothing (issue 222): it is asked
    // about...
    assert!(ended_ids(&value, &listed).is_empty());
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    assert_eq!(unlisted(&value, &view, &listed), ["t1"]);
    // ...and `terminal.get` answering `not_found` ends it.
    listed.not_found("t1");
    assert_eq!(ended_ids(&value, &listed), ["sp1"]);
}

#[test]
fn not_found_ends_only_a_terminal_the_list_left_out() {
    // A listed terminal, live, is never ended by a `not_found` for it.
    let mut listed = listing(&[("t1", "live", None)]);
    listed.not_found("t1");
    assert!(ended_ids(&one("t1"), &listed).is_empty());
    // Another machine's terminal is never asked about nor ended.
    let mut listed = listing(&[]);
    let value = one("box/t1");
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    assert!(unlisted(&value, &view, &listed).is_empty());
    listed.not_found("box/t1");
    assert!(ended_ids(&value, &listed).is_empty());
}

#[test]
fn unlisted_names_each_terminal_of_plain_tabs_once() {
    let with_mod = json!({"type": "row", "children": [block("t3"),
        {"type": "block", "id": "pm", "content": {"mod": "notes", "props": {}}}]});
    let value = doc(vec![
        space(
            "sp1",
            vec![tab("tb1", block("t1")), tab("tb2", block("t2"))],
        ),
        space("sp2", vec![tab("tb3", block("t1")), tab("tb4", with_mod)]),
    ]);
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    let listed = listing(&[("t2", "live", None)]);
    // t1 once, t2 listed, t3 in a tab that is not plain.
    assert_eq!(unlisted(&value, &view, &listed), ["t1"]);
    // The control: t3 is a terminal of the view, left out of the list.
    assert!(view.spaces[1].tabs[1].terminals.contains(&"t3".to_owned()));
}

#[test]
fn live_terminal_keeps_its_space() {
    // The negative control of the three above.
    let found = ended_ids(&one("t1"), &listing(&[("t1", "live", None)]));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn space_with_a_live_terminal_is_pruned_not_removed() {
    let value = doc(vec![space(
        "sp1",
        vec![
            tab("tb1", block("t1")),
            tab(
                "tb2",
                json!({"type": "row", "id": "tb2", "children": [block("t2"), block("t3")]}),
            ),
        ],
    )]);
    let listed = listing(&[
        ("t1", "finished", Some("killed")),
        ("t2", "finished", Some("exited")),
        ("t3", "live", None),
    ]);
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    let found = ended(&value, &view, &listed);
    // Its ended terminals go; the live one is never named.
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].id, "sp1");
    assert_eq!(found[0].ended, ["t1", "t2"]);
}

#[test]
fn unkept_terminal_stopped_with_its_server_ends_its_space() {
    // `hc server stop` or a restart: herdr-core never starts it again.
    let found = ended_ids(
        &one("t1"),
        &listing(&[("t1", "finished", Some("server_stop"))]),
    );
    assert_eq!(found, ["sp1"]);
}

#[test]
fn kept_terminal_or_another_reason_keeps_its_space() {
    // A kept terminal gets a shell at the next start; `holder_lost` and an
    // unknown reason are not ends either.
    let kept = |reason: &str| {
        let terminal = serde_json::from_value::<Terminal>(
            json!({"address": "t1", "state": "finished", "reason": reason, "keep": true}),
        )
        .unwrap_or_else(|e| panic!("terminal: {e}"));
        Listing::new(&[terminal])
    };
    for (what, listing) in [
        ("kept server_stop", kept("server_stop")),
        ("kept holder_lost", kept("holder_lost")),
        (
            "holder_lost",
            listing(&[("t1", "finished", Some("holder_lost"))]),
        ),
        (
            "newer reason",
            listing(&[("t1", "finished", Some("parked"))]),
        ),
        ("no reason", listing(&[("t1", "finished", None)])),
    ] {
        let found = ended_ids(&one("t1"), &listing);
        assert!(found.is_empty(), "{what}: {found:?}");
    }
}

#[test]
fn other_machines_terminal_is_never_judged_absent() {
    assert!(ended_ids(&one("box/t1"), &listing(&[])).is_empty());
    let found = ended_ids(
        &one("box/t1"),
        &listing(&[("box/t1", "finished", Some("exited"))]),
    );
    assert_eq!(found, ["sp1"], "a listed end counts on any machine");
}

#[test]
fn tab_holding_a_mod_keeps_its_space() {
    let value = doc(vec![space(
        "sp1",
        vec![tab(
            "tb1",
            json!({"type": "row", "children": [block("t1"),
                {"type": "block", "id": "pm", "content": {"mod": "notes", "props": {}}}]}),
        )],
    )]);
    let found = ended_ids(&value, &listing(&[("t1", "finished", Some("exited"))]));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn ended_terminals_of_a_tab_that_is_not_plain_are_not_named() {
    let value = doc(vec![space(
        "sp1",
        vec![
            tab(
                "tb1",
                json!({"type": "row", "children": [block("t1"),
                    {"type": "block", "id": "pm", "content": {"mod": "notes", "props": {}}}]}),
            ),
            tab("tb2", block("t2")),
        ],
    )]);
    let listed = listing(&[
        ("t1", "finished", Some("exited")),
        ("t2", "finished", Some("exited")),
    ]);
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    let found = ended(&value, &view, &listed);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].ended, ["t2"], "the mod tab's terminal stays");
}

#[test]
fn space_without_tabs_or_terminals_is_ended() {
    let value = doc(vec![
        space("sp1", vec![]),
        space("sp2", vec![json!({"id": "tb2", "name": ""})]),
    ]);
    assert_eq!(ended_ids(&value, &listing(&[])), ["sp1", "sp2"]);
}

#[test]
fn space_without_id_is_left_alone() {
    let value = doc(vec![json!({"name": "", "tabs": [tab("tb1", block("t1"))]})]);
    assert!(ended_ids(&value, &listing(&[])).is_empty());
}

#[test]
fn found_space_is_its_value_as_read() {
    let mut value = one("t1");
    value["spaces"][0]["newer"] = json!([1, 2]);
    let view = parse_view(&value).unwrap_or_else(|e| panic!("view: {e}"));
    let mut listed = listing(&[]);
    listed.not_found("t1");
    let found = ended(&value, &view, &listed);
    assert_eq!(
        found,
        [Candidate {
            id: "sp1".into(),
            space: value["spaces"][0].clone(),
            ended: vec!["t1".into()],
        }]
    );
}

fn candidate() -> Vec<Candidate> {
    vec![Candidate {
        id: "sp1".into(),
        space: one("t1")["spaces"][0].clone(),
        ended: vec!["t1".into()],
    }]
}

fn removed(intents: &[Intent]) -> Vec<&str> {
    intents
        .iter()
        .filter_map(|intent| match intent {
            Intent::PruneSpace { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn first_read_of_a_boot_never_removes() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    assert!(tracker.observe("c#1", now, candidate()).is_empty());
}

#[test]
fn second_read_after_the_grace_removes() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    tracker.observe("c#1", now, candidate());
    let early = tracker.observe("c#1", now + GRACE - Duration::from_millis(1), candidate());
    assert!(early.is_empty(), "within the grace: {early:?}");
    let due = tracker.observe("c#1", now + GRACE, candidate());
    assert_eq!(removed(&due), ["sp1"]);
    let Some(Intent::PruneSpace { space, ended, .. }) = due.first() else {
        panic!("no removal");
    };
    assert_eq!(*space, candidate()[0].space, "the space as read");
    assert_eq!(*ended, ["t1"]);
}

#[test]
fn another_boot_starts_over() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    tracker.observe("c#1", now, candidate());
    // Same core id, a new connection: a restarted core, or a relaunch.
    let after = tracker.observe("c#2", now + GRACE * 3, candidate());
    assert!(after.is_empty(), "{after:?}");
    let due = tracker.observe("c#2", now + GRACE * 4, candidate());
    assert_eq!(removed(&due), ["sp1"]);
}

#[test]
fn failed_read_forgets_every_candidate() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    tracker.observe("c#1", now, candidate());
    tracker.forget();
    let after = tracker.observe("c#1", now + GRACE * 2, candidate());
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn space_seen_alive_in_between_starts_over() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    tracker.observe("c#1", now, candidate());
    tracker.observe("c#1", now + GRACE, Vec::new());
    let after = tracker.observe("c#1", now + GRACE * 2, candidate());
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn other_ended_terminals_in_between_start_over() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    let mut more = candidate();
    more[0].ended = vec!["t1".into(), "t2".into()];
    tracker.observe("c#1", now, candidate());
    let after = tracker.observe("c#1", now + GRACE, more);
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn space_changed_in_between_starts_over() {
    let mut tracker = Tracker::new(GRACE);
    let now = Instant::now();
    tracker.observe("c#1", now, candidate());
    let mut renamed = candidate();
    renamed[0].space["name"] = json!("Build");
    let after = tracker.observe("c#1", now + GRACE, renamed.clone());
    assert!(after.is_empty(), "{after:?}");
    let due = tracker.observe("c#1", now + GRACE * 2, renamed);
    assert_eq!(removed(&due), ["sp1"]);
}
