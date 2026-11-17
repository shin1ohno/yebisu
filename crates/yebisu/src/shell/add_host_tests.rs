//! Model rules of the Add host sheet beyond the plan v2 7.12 names (those are
//! in `hosts_sheet_tests/`): widths, editing, modes and the notices.
use super::*;

const HOST: &str = "user@pro-dev.home.local";
const FINGERPRINT: &str = "ED25519 SHA256:JOTFRvCkuEl2HbyffolO4A+q4cUOVf0UHhWQLrlaEiU";

#[test]
fn sheet_is_620_wide_or_the_window_minus_32() {
    assert_eq!(sheet_width(1376.0), 620.0);
    assert_eq!(sheet_width(652.0), 620.0);
    assert_eq!(sheet_width(605.0), 573.0, "Split View half");
    assert_eq!(sheet_width(400.0), 368.0);
    assert_eq!(sheet_width(10.0), 0.0, "never negative");
}

#[test]
fn sheet_height_stays_above_the_keyboard() {
    // The input is the visible height, keyboard already excluded
    // (Window::fully_visible_bounds): 834 pt with no keyboard, then with a
    // 320 pt keyboard up.
    assert_eq!(sheet_height(834.0), 802.0);
    assert_eq!(sheet_height(834.0 - 320.0), 482.0);
    assert_eq!(sheet_height(20.0), 0.0, "never negative");
}

#[test]
fn empty_host_cannot_be_scanned() {
    let mut flow = AddHostFlow::default();
    assert!(!flow.can_scan());
    assert!(flow.begin_scan().is_none());
    assert!(flow.edit_host("   "));
    assert!(!flow.can_scan(), "blanks are not a host");
    assert!(flow.edit_host(HOST));
    assert!(flow.can_scan());
}

#[test]
fn long_host_name_is_kept_whole() {
    let long = format!("{}@{}.example.com", "u".repeat(10), "h".repeat(40));
    let mut flow = AddHostFlow::default();
    assert!(flow.edit_host(&long));
    assert_eq!(flow.begin_scan().as_deref(), Some(long.as_str()));
}

#[test]
fn editing_is_refused_while_a_job_runs_and_after_trust() {
    let mut flow = AddHostFlow::default();
    flow.edit_host(HOST);
    assert!(flow.begin_scan().is_some());
    assert!(!flow.edit_host("other@host"), "scan running");
    assert_eq!(flow.typed_host, HOST);
    flow.finish_scan(
        HOST,
        &ScanOutcome::Pinned {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert_eq!(flow.step, AddHostStep::SignIn);
    assert!(!flow.edit_host("other@host"), "host is trusted");
    assert_eq!(flow.typed_host, HOST);
}

#[test]
fn pinned_host_skips_the_comparison() {
    let mut session = AddHostSession::new(SheetMode::Add, HOST);
    session.begin_scan();
    session.finish_scan(
        HOST,
        ScanOutcome::Pinned {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert_eq!(session.flow.step, AddHostStep::SignIn);
    assert!(!session.flow.can_trust(), "nothing to trust, it is pinned");
    assert!(session.can_sign_in());
}

#[test]
fn trust_failure_returns_to_the_address_with_a_reason() {
    let mut session = AddHostSession::new(SheetMode::Add, HOST);
    session.begin_scan();
    session.finish_scan(
        HOST,
        ScanOutcome::New {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert!(session.flow.begin_trust());
    session.finish_trust(Err("disk full".into()));
    assert_eq!(session.flow.step, AddHostStep::Address);
    assert_eq!(session.flow.typed_host, HOST);
    assert_eq!(
        session.notice,
        Some(Notice::TrustFailed("disk full".into()))
    );
    // A trust reply with no trust running changes nothing.
    session.finish_trust(Ok(()));
    assert_eq!(session.flow.step, AddHostStep::Address);
}

#[test]
fn successful_trust_moves_to_sign_in() {
    let mut session = AddHostSession::new(SheetMode::Add, HOST);
    session.begin_scan();
    session.finish_scan(
        HOST,
        ScanOutcome::New {
            fingerprint: FINGERPRINT.into(),
        },
    );
    assert!(session.flow.begin_trust());
    session.finish_trust(Ok(()));
    assert_eq!(session.flow.step, AddHostStep::SignIn);
    assert!(session.notice.is_none());
}

#[test]
fn open_mode_starts_at_sign_in_and_edit_mode_at_the_address() {
    let open = AddHostSession::new(SheetMode::Open, HOST);
    assert_eq!(open.flow.step, AddHostStep::SignIn);
    assert_eq!(open.mode.title(), "Open host");
    let edit = AddHostSession::new(SheetMode::Edit, HOST);
    assert_eq!(edit.flow.step, AddHostStep::Address);
    assert_eq!(edit.flow.typed_host, HOST);
    assert_eq!(edit.mode.title(), "Edit host");
    assert_eq!(AddHostSession::default().mode.title(), "Add host");
}

#[test]
fn second_sign_in_is_refused_while_one_runs() {
    let mut session = AddHostSession::new(SheetMode::Open, HOST);
    assert_eq!(session.begin_sign_in().as_deref(), Some(HOST));
    assert!(!session.can_sign_in());
    assert!(session.begin_sign_in().is_none());
    assert_eq!(
        session.finish_sign_in(Ok(Probe {
            herdr1: true,
            ..Probe::default()
        })),
        Some(HerdrKind::Herdr1)
    );
    assert!(
        session.finish_sign_in(Ok(Probe::default())).is_none(),
        "stale"
    );
}

#[test]
fn outdated_herdr_is_reported_with_retry() {
    let mut session = AddHostSession::new(SheetMode::Open, HOST);
    session.begin_sign_in();
    let opened = session.finish_sign_in(Ok(Probe {
        outdated: true,
        ..Probe::default()
    }));
    assert!(opened.is_none());
    assert_eq!(session.notice, Some(Notice::HerdrOutdated));
    assert!(session.notice.as_ref().is_some_and(Notice::offers_retry));
}
