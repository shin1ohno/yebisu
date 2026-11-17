use super::*;
use crate::Error;
use crate::setup::Reply;
use crate::shell::add_host::{AddHostStep, Notice};

#[test]
fn scan_in_flight_blocks_second_scan_and_trust() {
    let mut state = scanning_state();
    let Some(session) = state.add.as_mut() else {
        panic!("sheet is open");
    };
    assert!(session.flow.scanning);
    assert!(!session.flow.can_scan(), "second Scan while one runs");
    assert!(session.begin_scan().is_none());
    // Even with a fingerprint on screen, Trust waits for the running job.
    session.flow.step = AddHostStep::Trust;
    session.flow.scanned_fingerprint = Some(FINGERPRINT.into());
    assert!(!session.flow.can_trust(), "Trust while a job runs");
    assert!(!session.flow.begin_trust());
    // A trust in flight blocks a second Trust and a Scan.
    session.flow.scanning = false;
    assert!(session.flow.begin_trust());
    assert!(!session.flow.can_trust() && !session.flow.can_scan());
    assert!(!session.flow.begin_trust());
}

#[test]
fn cancel_stops_the_scan_and_keeps_the_host() {
    let mut state = scanning_state();
    let Some(session) = state.add.as_mut() else {
        panic!("sheet is open");
    };
    session.cancel_scan();
    assert!(!session.flow.scanning);
    assert_eq!(session.flow.typed_host, HOST);
    assert_eq!(session.notice, Some(Notice::Cancelled));
    assert!(session.flow.can_scan(), "a new Scan may start");
    // The cancelled scan answers late: nothing is running, so it is stale.
    session.finish_scan(
        HOST,
        ScanOutcome::New {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert_eq!(session.flow.step, AddHostStep::Address);
    assert!(session.flow.scanned_fingerprint.is_none());
}

#[test]
fn failed_scan_keeps_typed_host() {
    let mut state = scanning_state();
    state.observe(&Err(Error::Ssh("the host is unreachable".into())));
    let Some(session) = state.add.as_ref() else {
        panic!("sheet is open");
    };
    assert_eq!(session.flow.typed_host, HOST, "the typed host stays");
    assert!(!session.flow.scanning);
    assert_eq!(session.flow.step, AddHostStep::Address);
    let lines = session
        .notice
        .as_ref()
        .map(Notice::lines)
        .unwrap_or_default();
    assert!(lines.iter().any(|l| l.contains("the host is unreachable")));
    assert!(session.notice.as_ref().is_some_and(Notice::offers_retry));
    assert!(session.flow.can_scan(), "Retry scans the same host");
}

#[test]
fn mismatch_shows_both_fingerprints() {
    let mut state = scanning_state();
    state.observe(&Ok(Reply::Mismatch {
        target: HOST.into(),
        expected: "ED25519 SHA256:EXPECTED".into(),
        seen: "ED25519 SHA256:SEEN".into(),
    }));
    let Some(session) = state.add.as_ref() else {
        panic!("sheet is open");
    };
    let lines = session
        .notice
        .as_ref()
        .map(Notice::lines)
        .unwrap_or_default();
    assert!(lines.iter().any(|l| l.contains("ED25519 SHA256:EXPECTED")));
    assert!(lines.iter().any(|l| l.contains("ED25519 SHA256:SEEN")));
    assert!(lines.iter().any(|l| l.contains("Refused")));
    assert!(!session.flow.can_trust(), "a changed key is never trusted");
    assert!(session.flow.scanned_fingerprint.is_none());
    assert!(!session.notice.as_ref().is_some_and(Notice::offers_retry));
    assert_eq!(session.flow.typed_host, HOST);
}
