//! The `herdr-app` document against a real herdr-core (#144 D5, issue
//! Verification "実 herdr-core に対する統合テスト"). Opt in with
//! `YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core cargo test -p yebisu
//! herdr_doc_live -- --ignored --test-threads=1`. The core is one server of
//! the private fleet of `core/space_fleet.rs` (its own home, state and
//! runtime folder, cleared environment) in the session `test-doc`, reached
//! through `hc connect` as the iPad reaches it; the test never reaches a
//! running core and ends only its own (`server stop` on that session).
use super::*;
use crate::core::{
    herdr_doc::{Intent, apply, check_write, parse_view},
    space_fleet::Fleet,
};
use serde_json::{Value, json};

const SESSION: &str = "test-doc";

fn refused_kind(error: CoreError) -> Option<String> {
    match error {
        CoreError::Refused { kind, .. } => kind,
        _ => None,
    }
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn herdr_doc_live_version_check_events_and_restart() {
    // The issue's guard: only a test session, never a real one.
    assert!(
        SESSION.starts_with("test-"),
        "HC_SESSION must start with test-"
    );
    let mut fleet = Fleet::start();
    fleet.add(SESSION);
    let (one, two) = (fleet.bridge(SESSION), fleet.bridge(SESSION));
    let (one, two) = (one.core(), two.core());
    // A document never written.
    let first = one
        .doc_get(herdr_doc::NAME)
        .unwrap_or_else(|e| panic!("get: {e}"));
    assert_eq!((first.value, first.version), (Value::Null, 0));
    // The other connection hears each write as a doc event.
    let mut events = two.subscribe().unwrap_or_else(|e| panic!("subscribe: {e}"));
    let mut written: Value =
        serde_json::from_str(include_str!("../../fixtures/herdr-app/unknown-fields.json"))
            .unwrap_or_else(|e| panic!("fixture: {e}"));
    let version = one
        .doc_set(herdr_doc::NAME, &written, 0)
        .unwrap_or_else(|e| panic!("set: {e}"));
    let heard = (0..40).any(|_| {
        events
            .next(Duration::from_millis(250))
            .unwrap_or_default()
            .iter()
            .any(|n| matches!(n, Notice::Doc { name, version: v, .. } if name == herdr_doc::NAME && *v == version))
    });
    assert!(heard, "no doc event for version {version}");
    // A stale version is refused and changes nothing.
    let stale = two.doc_set(herdr_doc::NAME, &json!({"spaces": []}), 0);
    assert_eq!(
        stale.map_err(refused_kind).err(),
        Some(Some("conflict".into()))
    );
    let read = two
        .doc_get(herdr_doc::NAME)
        .unwrap_or_else(|e| panic!("get: {e}"));
    assert_eq!(
        (read.value.clone(), read.version),
        (written.clone(), version)
    );
    // An intent applied to what was read passes the gates, is accepted, and
    // reads back as the official app's shape.
    let intent = Intent::NewTab {
        space: "sp00000000000000000000000000000001".into(),
        tab_id: "tblive".into(),
        pane_id: "plive".into(),
        terminal: "t99".into(),
    };
    let Ok(Applied::Changed(next)) = apply(&read.value, &intent) else {
        panic!("the intent changed nothing");
    };
    check_write(&read.value, &next, &intent).unwrap_or_else(|e| panic!("gates: {e}"));
    let after = two
        .doc_set(herdr_doc::NAME, &next, read.version)
        .unwrap_or_else(|e| panic!("set: {e}"));
    written = next;
    // More than 1 MiB is refused.
    let large = json!({"x": "x".repeat(herdr_doc::MAX_BYTES)});
    let refused = one.doc_set(herdr_doc::NAME, &large, after);
    assert_eq!(
        refused.map_err(refused_kind).err(),
        Some(Some("invalid_params".into()))
    );
    drop(events);
    // A restart keeps the value (unknown fields too) and its version.
    fleet.restart(SESSION);
    let back = fleet
        .bridge(SESSION)
        .core()
        .doc_get(herdr_doc::NAME)
        .unwrap_or_else(|e| panic!("get after restart: {e}"));
    assert_eq!((back.value.clone(), back.version), (written, after));
    parse_view(&back.value).unwrap_or_else(|e| panic!("readable after restart: {e}"));
    assert_eq!(back.value["x-top"], json!({"layout": {"gap": 4}}));
}
