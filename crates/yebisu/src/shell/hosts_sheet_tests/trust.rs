use super::*;
use crate::shell::add_host::AddHostStep;
use sheprd_client::ssh::{HostKeyFingerprint, NewHostKey};
use std::path::PathBuf;

fn pending_key() -> NewHostKey {
    NewHostKey::fixture_for_tests(
        PathBuf::from("known_hosts"),
        vec![HostKeyFingerprint {
            kind: "ED25519".into(),
            bits: 256,
            fingerprint: FINGERPRINT.into(),
        }],
    )
}

fn new_key() -> ScanOutcome {
    ScanOutcome::New {
        fingerprint: FINGERPRINT.into(),
    }
}

#[test]
fn trust_requires_successful_scan() {
    let mut session = AddHostSession::new(SheetMode::Add, HOST);
    assert!(!session.flow.can_trust(), "nothing scanned yet");
    assert!(session.begin_scan().is_some());
    assert!(!session.flow.can_trust(), "scan still running");
    session.finish_scan(HOST, ScanOutcome::Failed("unreachable".into()));
    assert!(!session.flow.can_trust(), "scan failed");
    assert!(session.begin_scan().is_some());
    // A reply for another host is stale and cannot unlock Trust.
    session.finish_scan("other@host", new_key());
    assert!(!session.flow.can_trust());
    session.finish_scan(HOST, new_key());
    assert!(session.flow.can_trust(), "scan succeeded for this host");
    // Editing the host drops the scan: the fingerprint belonged to the old text.
    assert!(session.flow.edit_host("user@elsewhere"));
    assert!(!session.flow.can_trust());
    assert_eq!(session.flow.step, AddHostStep::Address);
    // A step flag alone is not enough without a scanned fingerprint.
    session.flow.step = AddHostStep::Trust;
    assert!(!session.flow.can_trust());
}

#[test]
fn rejecting_the_key_returns_to_the_address() {
    let mut session = comparing();
    session.flow.reject_key();
    assert_eq!(session.flow.step, AddHostStep::Address);
    assert_eq!(session.flow.typed_host, HOST);
    assert!(!session.flow.can_trust());
}

#[gpui::test]
fn trust_is_bound_to_the_scanned_target(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.shell.hosts_sheet.add = Some(comparing());
            this.setup_input.set_text(HOST.into());
            // The pending key was scanned for another host: refused, no job.
            this.setup.pending = Some(("user@other.host".into(), pending_key()));
            this.hosts_trust(cx);
            assert!(!this.setup.busy, "a key scanned for another host");
            // Nothing pending at all: refused as well.
            this.setup.pending = None;
            this.hosts_trust(cx);
            assert!(!this.setup.busy);
            // The scanned target matches the field: the job starts once.
            this.setup.pending = Some((HOST.into(), pending_key()));
            this.hosts_trust(cx);
            assert!(this.setup.busy);
            let trusting = this
                .shell
                .hosts_sheet
                .add
                .as_ref()
                .is_some_and(|s| s.flow.scanning);
            assert!(trusting);
        });
    });
}
