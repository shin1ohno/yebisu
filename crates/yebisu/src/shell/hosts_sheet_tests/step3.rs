use super::*;
use crate::Error;
use crate::setup::Reply;
use crate::shell::{
    HerdrKind,
    add_host::{Notice, SignInOutcome, sign_in_outcome},
};

fn signing_state() -> HostsSheetState {
    let mut state = HostsSheetState::default();
    state.open_add(SheetMode::Open, HOST);
    let started = state.add.as_mut().and_then(|s| s.begin_sign_in());
    assert_eq!(started.as_deref(), Some(HOST));
    state
}

fn signed_in(probe: Probe) -> Reply {
    Reply::SignedIn(Default::default(), "SSH authenticated".into(), probe)
}

#[test]
fn step3_prefers_herdr1() {
    // Both daemons answer: Herdr 1 opens.
    assert_eq!(
        sign_in_outcome(probe(true, true, false)),
        SignInOutcome::Open(HerdrKind::Herdr1)
    );
    // Only Herdr 2 answers: Herdr 2 opens.
    assert_eq!(
        sign_in_outcome(probe(false, true, false)),
        SignInOutcome::Open(HerdrKind::Herdr2)
    );
    // Herdr 1 too old but Herdr 2 present: Herdr 2 opens.
    assert_eq!(
        sign_in_outcome(probe(false, true, true)),
        SignInOutcome::Open(HerdrKind::Herdr2)
    );
    assert_eq!(
        sign_in_outcome(probe(false, false, true)),
        SignInOutcome::Outdated
    );
    assert_eq!(
        sign_in_outcome(probe(false, false, false)),
        SignInOutcome::NoHerdr
    );
    // The same order through the reply that the sheet receives.
    let mut state = signing_state();
    state.observe(&Ok(signed_in(probe(true, true, false))));
    assert_eq!(state.pending_open, Some(HerdrKind::Herdr1));
    let mut state = signing_state();
    state.observe(&Ok(signed_in(probe(false, true, false))));
    assert_eq!(state.pending_open, Some(HerdrKind::Herdr2));
}

#[test]
fn herdr_that_does_not_answer_is_said_so() {
    let mut state = signing_state();
    state.observe(&Ok(signed_in(probe(false, false, false))));
    assert!(state.pending_open.is_none());
    let notice = state.add.as_ref().and_then(|s| s.notice.clone());
    assert_eq!(notice, Some(Notice::NoHerdr));
    let lines = notice.map(|n| n.lines()).unwrap_or_default();
    assert!(lines.iter().any(|l| l.contains("SSH connected")));
}

#[test]
fn failed_sign_in_shows_reason_and_retry() {
    let mut state = signing_state();
    state.observe(&Err(Error::Ssh("authentication refused: Auth".into())));
    let Some(session) = state.add.as_ref() else {
        panic!("sheet is open");
    };
    assert!(state.pending_open.is_none());
    assert!(matches!(&session.notice, Some(Notice::SignInFailed(r)) if r.contains("Auth")));
    assert!(session.notice.as_ref().is_some_and(Notice::offers_retry));
    assert!(session.can_sign_in(), "Retry signs in again");
}

#[gpui::test]
fn open_on_a_saved_host_signs_in_at_once(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.hosts_open_add(SheetMode::Open, HOST, cx);
            assert!(this.setup.busy, "the sign-in job started");
            let signing = this
                .shell
                .hosts_sheet
                .add
                .as_ref()
                .is_some_and(|s| s.signing_in);
            assert!(signing);
            assert_eq!(this.setup_input.text, HOST);
            // A second tap while the job runs starts nothing new.
            this.hosts_sign_in(cx);
            assert!(this.setup.busy);
        });
    });
}
