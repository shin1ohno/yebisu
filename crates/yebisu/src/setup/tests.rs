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

fn saved_hosts(selected: &str) -> crate::hosts::SavedHosts {
    crate::hosts::SavedHosts {
        targets: vec!["existing@host".into(), selected.into()],
        selected: selected.into(),
    }
}

#[test]
fn trust_reply_adds_host_to_setup_list_without_reload() {
    let mut setup = Setup::default();
    setup.apply(Ok(Reply::Saved(saved_hosts("trusted@host"), "pin".into())));
    assert_eq!(setup.hosts, ["existing@host", "trusted@host"]);
    assert_eq!(setup.target, "trusted@host");
    assert_eq!(setup.fingerprints, "pin");
    assert!(setup.status.contains("Host key trusted"));
    assert!(setup.pending.is_none());
}

#[test]
fn authentication_reply_adds_host_to_setup_list_without_reload() {
    let mut setup = Setup::default();
    setup.apply(Ok(Reply::Authenticated(
        saved_hosts("authenticated@host"),
        "SSH authenticated".into(),
    )));
    assert_eq!(setup.hosts, ["existing@host", "authenticated@host"]);
    assert_eq!(setup.target, "authenticated@host");
    assert_eq!(setup.status, "SSH authenticated");
    setup.apply(Err(Error::Storage(std::io::Error::other("save failed"))));
    assert_eq!(setup.hosts, ["existing@host", "authenticated@host"]);
}
