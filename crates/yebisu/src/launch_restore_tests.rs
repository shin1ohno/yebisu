//! A cold launch with saved hosts and a record of what was open restores it
//! (owner decision 2026-11-25). Every dial goes to an in-process peer; no
//! test reaches a host.
use super::*;
use crate::terminal_view::TerminalView;
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const ONE: &str = "qa@one.invalid";
const TWO: &str = "qa@two.invalid";
const UNREACHABLE: &str = "host unreachable in test";

fn herdr1(host: &str) -> Entry {
    Entry {
        host: host.into(),
        kind: HerdrKind::Herdr1,
        space: None,
    }
}

fn herdr2(host: &str, space: &str) -> Entry {
    Entry {
        host: host.into(),
        kind: HerdrKind::Herdr2,
        space: Some(space.into()),
    }
}

/// The state a launch reaches once `Job::Load` and the settings answered.
fn launch(this: &mut Smoke, record: Option<Record>, automatic: bool, probe: Arc<SpaceProbe>) {
    this.setup.hosts = vec![ONE.into(), TWO.into()];
    this.setup.loaded = true;
    this.terminal.reconnect.automatic = automatic;
    this.launch = LaunchRestore::loaded(record);
    this.launch.probe = Some(probe);
}

fn listed(spaces: &[&str]) -> Arc<SpaceProbe> {
    let spaces = spaces
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<BTreeSet<_>>();
    Arc::new(move |_| Ok(spaces.clone()))
}

fn unreachable() -> Arc<SpaceProbe> {
    Arc::new(|_| Err(crate::Error::Ssh(UNREACHABLE.into())))
}

fn never_called() -> Arc<SpaceProbe> {
    Arc::new(|host| panic!("no space check was expected for {host}"))
}

fn herdr1_peer() -> TerminalView {
    TerminalView {
        test_client: Some(crate::mock_peer::connect().unwrap_or_else(|e| panic!("peer: {e}"))),
        ..Default::default()
    }
}

fn core_peer() -> (
    sheprd_core_client::CoreBackend,
    std::sync::mpsc::Receiver<(u32, Vec<u8>)>,
) {
    crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("core peer: {e}"))
}

fn core_name(view: &TerminalView) -> Option<String> {
    view.core.as_ref().map(|core| core.name().to_owned())
}

#[track_caller]
fn wait_until(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    what: &str,
    predicate: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            predicate(this)
        }) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn smoke(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    cx.add_window_view(|_, cx| Smoke::new(cx))
}

#[gpui::test]
fn cold_launch_reopens_the_saved_herdr2_space_and_the_hidden_herdr1_host(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr2(TWO, "space-a")),
        others: vec![herdr1(ONE)],
    };
    view.update(cx, |this, _| {
        launch(
            this,
            Some(record.clone()),
            true,
            listed(&["default", "space-a"]),
        );
        this.board.hidden.insert(ONE.into(), herdr1_peer());
    });
    view.update(cx, |this, cx| this.poll_terminal(cx));
    view.update(cx, |this, _| {
        assert!(
            this.terminal.open,
            "the launch did not reopen the shown terminal"
        );
        assert_eq!(this.terminal.reconnect.target, TWO);
        assert!(
            this.terminal.core_mode,
            "the shown terminal must be Herdr 2"
        );
        let hidden = this.board.hidden.get(ONE).unwrap_or_else(|| panic!("ONE"));
        assert!(hidden.test_client.is_none(), "ONE's Herdr 1 was not dialed");
        assert!(
            this.launch.written().is_none(),
            "no write while a space is checked"
        );
    });
    let (backend, _wire) = core_peer();
    view.update(cx, |this, _| {
        this.terminal.core_test_backend = Some(backend)
    });
    cx.run_until_parked();
    wait_until(&view, cx, "the restored space's first frame", |this| {
        this.terminal.live.connected
    });
    view.update(cx, |this, cx| {
        assert_eq!(core_name(&this.terminal).as_deref(), Some("space-a"));
        this.poll_terminal(cx);
        assert_eq!(
            this.launch.written(),
            Some(&record),
            "the record is kept current"
        );
        this.terminal.core.take();
    });
}

#[gpui::test]
fn a_saved_space_that_is_gone_opens_default_with_a_notice(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr2(TWO, "space-a")),
        others: Vec::new(),
    };
    let (backend, _wire) = core_peer();
    view.update(cx, |this, cx| {
        launch(this, Some(record), true, listed(&["default"]));
        this.poll_terminal(cx);
        this.terminal.core_test_backend = Some(backend);
    });
    cx.run_until_parked();
    wait_until(&view, cx, "default's first frame", |this| {
        this.terminal.live.connected
    });
    view.update(cx, |this, _| {
        assert_eq!(core_name(&this.terminal).as_deref(), Some(HERDR2_SESSION));
        assert_eq!(this.shell.herdr2.selected_session(TWO), HERDR2_SESSION);
        let notice = this.terminal_status();
        assert!(notice.contains("space-a is gone"), "{notice}");
        this.terminal.core.take();
    });
}

#[gpui::test]
fn a_space_check_that_fails_shows_the_reason_and_waits_for_reconnect(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr2(TWO, "space-a")),
        others: Vec::new(),
    };
    view.update(cx, |this, cx| {
        launch(this, Some(record), true, unreachable());
        this.poll_terminal(cx);
    });
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(
            this.terminal.core.is_none(),
            "one try only: nothing dials after it"
        );
        assert!(!this.terminal.live.connected);
        let status = this.terminal_status();
        assert!(status.contains("space-a not restored") && status.contains(UNREACHABLE));
        // Not connected, nothing dialing: the status bar offers Reconnect.
        assert!(!this.chrome_state().connected);
        // Reconnect reopens the saved space, not `default`.
        assert_eq!(this.shell.herdr2.selected_session(TWO), "space-a");
    });
}

#[gpui::test]
fn tap_reconnect_shows_the_saved_terminal_without_dialing(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr2(TWO, "space-a")),
        others: vec![herdr1(ONE)],
    };
    view.update(cx, |this, cx| {
        launch(this, Some(record), false, never_called());
        this.poll_terminal(cx);
    });
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(this.terminal.open && this.terminal.core_mode);
        assert_eq!(this.terminal.reconnect.target, TWO);
        assert!(this.terminal.core.is_none(), "Tap reconnect must not dial");
        assert_eq!(this.terminal.live.status, TAP_RECONNECT);
        assert!(this.board.hidden.is_empty(), "no hidden host is dialed");
        assert_eq!(this.shell.herdr2.selected_session(TWO), "space-a");
    });
}

#[gpui::test]
fn the_shown_herdr1_host_and_a_hidden_herdr2_default_space_reopen(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr1(ONE)),
        others: vec![herdr2(TWO, HERDR2_SESSION)],
    };
    let (backend, _wire) = core_peer();
    view.update(cx, |this, cx| {
        launch(this, Some(record), true, never_called());
        this.board.hidden.insert(ONE.into(), herdr1_peer());
        let hidden = TerminalView {
            core_test_backend: Some(backend),
            ..Default::default()
        };
        this.board.hidden_core.insert(TWO.into(), hidden);
        this.poll_terminal(cx);
        assert!(this.terminal.open && !this.terminal.core_mode);
        assert_eq!(this.terminal.reconnect.target, ONE);
        assert!(
            this.terminal.test_client.is_none(),
            "ONE's Herdr 1 was not dialed"
        );
        let hidden = this
            .board
            .hidden_core
            .get(TWO)
            .unwrap_or_else(|| panic!("TWO"));
        assert_eq!(core_name(hidden).as_deref(), Some(HERDR2_SESSION));
        if let Some(hidden) = this.board.hidden_core.get_mut(TWO) {
            hidden.core.take();
        }
    });
}

#[gpui::test]
fn the_restore_waits_for_saved_hosts_and_skips_hosts_no_longer_saved(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr1("qa@removed.invalid")),
        others: vec![herdr1(ONE)],
    };
    view.update(cx, |this, cx| {
        launch(this, Some(record), true, never_called());
        this.setup.loaded = false;
        this.board.hidden.insert(ONE.into(), herdr1_peer());
        this.poll_terminal(cx);
        let waiting = this
            .board
            .hidden
            .get(ONE)
            .is_some_and(|t| t.test_client.is_some());
        assert!(waiting, "restored before the saved hosts were loaded");
        this.setup.loaded = true;
        this.poll_terminal(cx);
        assert!(
            !this.terminal.open,
            "a host that is no longer saved was opened"
        );
        let dialed = this
            .board
            .hidden
            .get(ONE)
            .is_some_and(|t| t.test_client.is_none());
        assert!(dialed, "the saved hidden host was not dialed");
    });
}

#[gpui::test]
fn a_host_opened_before_the_record_loads_is_not_replaced(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let record = Record {
        selected: Some(herdr2(TWO, "space-a")),
        others: Vec::new(),
    };
    view.update(cx, |this, cx| {
        launch(this, Some(record), true, never_called());
        this.terminal.open = true;
        this.terminal.reconnect.target = ONE.into();
        this.poll_terminal(cx);
        assert_eq!(this.terminal.reconnect.target, ONE);
        assert!(!this.terminal.core_mode);
    });
}

#[gpui::test]
fn the_record_is_not_written_before_the_restore_or_while_suspended(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    view.update(cx, |this, cx| {
        this.setup.hosts = vec![ONE.into(), TWO.into()];
        this.terminal.open = true;
        this.terminal.reconnect.target = ONE.into();
        this.poll_terminal(cx);
        assert!(this.launch.written().is_none(), "written before any launch");
        this.launch = LaunchRestore::loaded(None);
        this.setup.loaded = true;
        this.terminal.reconnect.suspended = true;
        this.poll_terminal(cx);
        assert!(this.launch.written().is_none(), "written while suspended");
        this.terminal.reconnect.suspended = false;
        this.poll_terminal(cx);
        this.poll_terminal(cx);
        let expected = Record {
            selected: Some(herdr1(ONE)),
            others: Vec::new(),
        };
        assert_eq!(this.launch.written(), Some(&expected));
    });
}

#[test]
fn a_space_whose_server_is_down_is_still_listed() {
    // The discovery poll's two output shapes: JSON rows (python3 on the host)
    // and the names-only listing of the state directories.
    let rows = b"[\"space-a\", null]\n[\"default\", \"unknown\"]\n";
    let names = crate::core::listed_spaces(rows);
    assert!(
        names.contains("space-a") && names.contains("default"),
        "{names:?}"
    );
    let names = crate::core::listed_spaces(b"\0space-a\0default\0");
    assert!(
        names.contains("space-a") && names.contains("default"),
        "{names:?}"
    );
    assert!(crate::core::listed_spaces(b"\0").is_empty());
}
