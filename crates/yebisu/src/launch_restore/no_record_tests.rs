//! A launch with no record opens Herdr 2 `default` on each saved host that
//! lists a Herdr 2 space (owner decision 2026-11-26, option A). The space
//! listing is the test probe and every dial goes to an in-process peer; no
//! test reaches a host.
use super::*;
use crate::shell::sidebar::{model::HostLink, view::host_entries};
use std::sync::atomic::{AtomicUsize, Ordering};

/// `ONE` lists a Herdr 2 space, `TWO` lists none; counts every listing.
fn one_lists_a_space(calls: Arc<AtomicUsize>) -> Arc<SpaceProbe> {
    Arc::new(move |host| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(if host == ONE {
            BTreeSet::from(["space-a".to_owned()])
        } else {
            BTreeSet::new()
        })
    })
}

fn counted_unreachable(calls: Arc<AtomicUsize>) -> Arc<SpaceProbe> {
    Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(crate::Error::Ssh(UNREACHABLE.into()))
    })
}

/// `ONE`'s hidden Herdr 2 view, with the in-process peer it will dial.
fn hidden_core_peer(this: &mut Smoke) -> std::sync::mpsc::Receiver<(u32, Vec<u8>)> {
    let (backend, wire) = core_peer();
    let hidden = TerminalView {
        core_test_backend: Some(backend),
        ..Default::default()
    };
    this.board.hidden_core.insert(ONE.into(), hidden);
    wire
}

#[gpui::test]
fn a_launch_with_no_record_opens_default_where_herdr2_lists_a_space(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let calls = Arc::new(AtomicUsize::new(0));
    let _wire = view.update(cx, |this, cx| {
        launch(this, None, true, one_lists_a_space(calls.clone()));
        let wire = hidden_core_peer(this);
        this.poll_terminal(cx);
        assert!(
            this.launch.written().is_none(),
            "no write while the hosts list their spaces"
        );
        wire
    });
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        for _ in 0..3 {
            this.poll_terminal(cx);
        }
        let hidden = this.board.hidden_core.get(ONE);
        assert_eq!(
            hidden.and_then(core_name).as_deref(),
            Some(HERDR2_SESSION),
            "ONE lists a Herdr 2 space: its default was not dialed"
        );
        assert!(
            !this.board.hidden_core.contains_key(TWO),
            "TWO lists no Herdr 2 space: nothing is dialed there"
        );
        assert!(!this.terminal.open, "default opens hidden, not shown");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "one listing per host");
        let expected = Record {
            selected: None,
            others: vec![herdr2(ONE, HERDR2_SESSION)],
        };
        assert_eq!(this.launch.written(), Some(&expected));
        if let Some(hidden) = this.board.hidden_core.get_mut(ONE) {
            hidden.core.take();
        }
    });
}

#[gpui::test]
fn a_record_that_lists_no_connection_dials_nothing(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    view.update(cx, |this, cx| {
        launch(this, Some(Record::default()), true, never_called());
        this.poll_terminal(cx);
    });
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        assert!(this.board.hidden_core.is_empty() && this.board.hidden.is_empty());
        assert!(!this.terminal.open);
        assert_eq!(this.launch.written(), Some(&Record::default()));
    });
}

#[gpui::test]
fn a_failed_listing_shows_the_reason_and_waits_for_reconnect(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    let calls = Arc::new(AtomicUsize::new(0));
    view.update(cx, |this, cx| {
        launch(this, None, true, counted_unreachable(calls.clone()));
        this.setup.hosts = vec![ONE.into()];
        this.poll_terminal(cx);
    });
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        for _ in 0..3 {
            this.poll_terminal(cx);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one try only");
        let hidden = this
            .board
            .hidden_core
            .get(ONE)
            .unwrap_or_else(|| panic!("ONE has no Herdr 2 row"));
        assert!(hidden.core.is_none(), "nothing dials after a failed try");
        assert!(
            hidden.live.status.contains(UNREACHABLE),
            "{}",
            hidden.live.status
        );
        let rows = host_entries(&this.shell.sidebar, &this.terminal, &this.board.hidden_core);
        let row = rows
            .iter()
            .find(|row| row.host == ONE && row.kind == HerdrKind::Herdr2)
            .unwrap_or_else(|| panic!("no Herdr 2 host row: {rows:?}"));
        assert!(matches!(row.link, HostLink::Offline(_)), "{:?}", row.link);
        // Tapping the row shows it; the status bar offers Reconnect.
        this.select_host_kind(ONE, HerdrKind::Herdr2);
        assert!(this.chrome_state().offers_reconnect());
        assert_eq!(this.shell.herdr2.selected_session(ONE), HERDR2_SESSION);
    });
}

#[gpui::test]
fn tap_reconnect_with_no_record_lists_and_dials_nothing(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx);
    view.update(cx, |this, cx| {
        launch(this, None, false, never_called());
        this.poll_terminal(cx);
    });
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        assert!(this.board.hidden_core.is_empty(), "Tap reconnect dialed");
        assert!(!this.terminal.open);
    });
}

#[test]
fn an_unreadable_record_counts_as_no_record() {
    let root = std::env::temp_dir().join(format!("yebisu-no-record-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(usable(store::load(&root)), None, "a missing file");
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("dir: {e}"));
    std::fs::write(root.join("yebisu-connections.json"), b"{")
        .unwrap_or_else(|e| panic!("corrupt: {e}"));
    assert!(store::load(&root).is_err());
    assert_eq!(usable(store::load(&root)), None, "a corrupt file");
    let _ = std::fs::remove_dir_all(&root);
}
