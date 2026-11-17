//! L7 tests: the rules of plan v2 7.12, one topic file each. They use the
//! state and the flow directly; only the opening rules need a window.
use super::*;
use crate::app::Smoke;
use crate::shell::add_host::{AddHostSession, Probe, ScanOutcome, SheetMode};
use gpui::{Entity, TestAppContext, VisualTestContext};

#[path = "hosts_sheet_tests/developer.rs"]
mod developer;
#[path = "hosts_sheet_tests/first_run.rs"]
mod first_run;
#[path = "hosts_sheet_tests/footer.rs"]
mod footer;
#[path = "hosts_sheet_tests/scan.rs"]
mod scan;
#[path = "hosts_sheet_tests/step3.rs"]
mod step3;
#[path = "hosts_sheet_tests/trust.rs"]
mod trust;

const HOST: &str = "user@pro-dev.home.local";
const FINGERPRINT: &str = "ED25519 SHA256:JOTFRvCkuEl2HbyffolO4A+q4cUOVf0UHhWQLrlaEiU";

fn window(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    cx.add_window_view(|_, cx| Smoke::new(cx))
}

/// A session that scanned `HOST` and waits for the user to compare.
fn comparing() -> AddHostSession {
    let mut session = AddHostSession::new(SheetMode::Add, HOST);
    assert_eq!(session.begin_scan().as_deref(), Some(HOST));
    session.finish_scan(
        HOST,
        ScanOutcome::New {
            fingerprint: FINGERPRINT.into(),
        },
    );
    session
}

/// A state whose Add host sheet is mid-scan of `HOST`.
fn scanning_state() -> HostsSheetState {
    let mut state = HostsSheetState::default();
    state.open_add(SheetMode::Add, HOST);
    let started = state.add.as_mut().and_then(|s| s.begin_scan());
    assert_eq!(started.as_deref(), Some(HOST));
    state
}

fn probe(herdr1: bool, herdr2: bool, outdated: bool) -> Probe {
    Probe {
        herdr1,
        herdr2,
        outdated,
    }
}
