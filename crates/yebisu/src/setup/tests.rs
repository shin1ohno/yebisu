use super::*;
use crate::Error;
use sheprd_client::ssh::SshFailure;

#[test]
fn save_failure_is_visible_and_clears_pending_trust() {
    let mut setup = Setup {
        busy: true,
        ..Setup::default()
    };
    setup.apply(Err(Error::Storage(std::io::Error::other(
        "disk-full-negative-control",
    ))));
    assert!(!setup.busy);
    assert!(setup.status.contains("disk-full-negative-control"));
    assert!(setup.status.contains("could not save or read"));
    assert!(setup.pending.is_none());
}

#[test]
fn changed_host_is_refused_without_showing_a_fingerprint() {
    let mut setup = Setup::default();
    setup.apply(Ok(Reply::Scanned(
        "a@mac".into(),
        HostKeyScan::Refused(SshFailure::HostKey),
        String::new(),
    )));
    assert!(setup.status.contains("Host refused"));
    assert!(setup.pending.is_none());
    assert!(setup.fingerprints.is_empty());
}

#[test]
fn invalid_key_is_not_displayed() {
    let mut setup = Setup::default();
    setup.apply(Err(Error::InvalidKeyLine));
    assert!(setup.key_line.is_none());
    assert!(setup.status.contains("unexpected key comment"));
}
