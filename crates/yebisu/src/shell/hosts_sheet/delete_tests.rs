//! #250: Delete Host from a row's left swipe and from the Edit sheet, both
//! confirmed; a connected host disconnects first and the selection moves on.
use super::*;
use crate::{
    hosts::SavedHosts,
    mock_peer,
    setup::Reply,
    shell::{add_host::SheetMode, hosts_sheet::delete_view::SWIPE_DELETE_WIDTH},
};
use gpui::{
    Entity, Modifiers, ScrollDelta, ScrollEvent, ScrollSource, TestAppContext, TouchPhase,
    VisualTestContext, point, size,
};
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};

/// The saved-host QA fixture: its row and Delete carry QA identifiers.
const A: &str = "qa@qa-alpha.invalid";
const B: &str = "dev@mini.local";
const C: &str = "dev@studio.local";

fn window<'a>(
    cx: &'a mut TestAppContext,
    hosts: &[&str],
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    let hosts = hosts
        .iter()
        .map(|host| (*host).to_owned())
        .collect::<Vec<_>>();
    view.update(cx, |this, cx| {
        this.apply_setup_result(Ok(Reply::Hosts(
            SavedHosts {
                selected: hosts.first().cloned().unwrap_or_default(),
                targets: hosts,
                names: [(B.to_owned(), "Mini".to_owned())].into(),
            },
            String::new(),
        )));
        this.setup.loaded = true;
        this.shell.hosts_sheet.auto_opened = true;
        this.open_hosts(cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// Shows `host` with a live Herdr 1 connection; returns its stop flag,
/// which a disconnect sets to false.
fn connect_shown(view: &Entity<Smoke>, cx: &mut VisualTestContext, host: &str) -> Arc<AtomicBool> {
    let (client, _) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host(host);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
        this.terminal.link.notification_active.clone()
    })
}

fn hidden_peer() -> (TerminalView, Arc<AtomicBool>) {
    let (client, _) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    let mut view = TerminalView::default();
    let generation = view.link.reset();
    view.link.install(generation, Ok(client));
    let active = view.link.notification_active.clone();
    (view, active)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not drawn"))
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = bounds(cx, selector).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// One touch pan as GPUI reports it: a step, then the release.
fn pan(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, x: f32, y: f32) {
    for (phase, delta) in [
        (TouchPhase::Started, point(px(x), px(y))),
        (TouchPhase::Ended, point(px(0.), px(0.))),
    ] {
        cx.simulate_event(ScrollEvent {
            position: at,
            delta: ScrollDelta::Pixels(delta),
            modifiers: Modifiers::none(),
            touch_phase: phase,
            source: ScrollSource::Touch,
        });
        cx.run_until_parked();
    }
    // The settle runs after the event.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn row_offset(view: &Entity<Smoke>, cx: &mut VisualTestContext, host: &str) -> f32 {
    view.update(cx, |this, _| this.shell.hosts_sheet.swipes.offset(host))
        .unwrap_or_else(|| panic!("{host} has no row"))
}

fn list_offset(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> f32 {
    view.update(cx, |this, _| {
        f32::from(this.shell.indicators.hosts_list.handle().offset().y)
    })
}

#[test]
fn a_row_more_than_half_open_settles_open_and_any_other_closes() {
    let width = SWIPE_DELETE_WIDTH;
    assert_eq!(settle(-width, width), -width);
    assert_eq!(settle(-width / 2. - 1., width), -width);
    assert_eq!(settle(-width / 2., width), 0.);
    assert_eq!(settle(-1., width), 0.);
    assert_eq!(settle(0., width), 0.);
    assert_eq!(settle(-10., 0.), 0., "a row with no room to scroll");
}

#[test]
fn only_a_connected_host_is_told_it_disconnects_first() {
    let mut confirm = DeleteConfirm {
        host: B.into(),
        name: "Mini".into(),
        connected: false,
    };
    assert_eq!(confirm.title(), "Delete Mini?");
    let says_disconnect = |confirm: &DeleteConfirm| {
        confirm
            .lines()
            .iter()
            .any(|line| line.contains("disconnects"))
    };
    assert!(!says_disconnect(&confirm));
    assert!(
        confirm
            .lines()
            .iter()
            .any(|line| line.contains("authorized_keys")),
        "the host keeps this iPad's public key"
    );
    confirm.connected = true;
    assert!(says_disconnect(&confirm));
}

#[gpui::test]
fn a_left_swipe_shows_the_rows_delete_and_a_vertical_pan_still_scrolls(cx: &mut TestAppContext) {
    // Enough hosts that the list scrolls.
    let many: Vec<String> = (0..30).map(|i| format!("dev@host-{i}.local")).collect();
    let mut hosts: Vec<&str> = vec![A];
    hosts.extend(many.iter().map(String::as_str));
    let (view, cx) = window(cx, &hosts);
    let panel = bounds(cx, "hosts-panel");
    let row = bounds(cx, "hosts-row-0");
    let at = point(row.left() + px(40.), row.center().y);
    assert!(
        bounds(cx, "host-delete-0").left() >= row.right(),
        "Delete starts past the row's edge"
    );

    pan(cx, at, -(SWIPE_DELETE_WIDTH + 30.), 0.);
    assert_eq!(row_offset(&view, cx, A), -SWIPE_DELETE_WIDTH);
    assert_eq!(
        list_offset(&view, cx),
        0.,
        "a sideways swipe moved the list"
    );
    let delete = bounds(cx, "host-delete-0");
    assert!(delete.right() <= panel.right(), "Delete is on screen");
    assert!(delete.left() >= panel.left());

    // A short swipe right closes the row again; a long left one reopens it.
    pan(cx, at, SWIPE_DELETE_WIDTH / 2. + 10., 0.);
    assert_eq!(row_offset(&view, cx, A), 0.);
    pan(cx, at, -(SWIPE_DELETE_WIDTH / 2. + 10.), 0.);
    assert_eq!(row_offset(&view, cx, A), -SWIPE_DELETE_WIDTH);

    pan(cx, at, 0., -120.);
    assert!(
        list_offset(&view, cx) < 0.,
        "a vertical pan on a row scrolls the list"
    );
    assert_eq!(row_offset(&view, cx, A), -SWIPE_DELETE_WIDTH);
}

#[gpui::test]
fn a_rows_delete_asks_first_and_cancel_changes_nothing(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A, B]);
    let row = bounds(cx, "hosts-row-0");
    pan(cx, point(row.left() + px(40.), row.center().y), -200., 0.);
    click(cx, "host-delete-0");
    view.update(cx, |this, _| {
        let confirm = this.shell.hosts_sheet.delete.clone();
        assert_eq!(confirm.as_ref().map(|c| c.host.as_str()), Some(A));
        assert_eq!(confirm.map(|c| c.connected), Some(false));
        assert!(!this.setup.busy, "nothing starts before the answer");
    });
    assert!(cx.debug_bounds("host-delete-confirm-card").is_some());
    click(cx, "host-delete-cancel");
    view.update(cx, |this, _| {
        assert!(this.shell.hosts_sheet.delete.is_none());
        assert!(!this.setup.busy);
        assert_eq!(this.setup.hosts, vec![A.to_owned(), B.to_owned()]);
        assert!(this.shell.hosts_sheet.list_open);
    });
    assert_eq!(row_offset(&view, cx, A), 0., "Cancel closes the row");
    assert!(cx.debug_bounds("host-delete-confirm-card").is_none());
}

#[gpui::test]
fn the_edit_sheet_deletes_the_host_it_opened_for(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A, B, C]);
    view.update(cx, |this, cx| this.hosts_open_add(SheetMode::Add, "", cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("host-edit-delete").is_none(),
        "Add has no Delete Host"
    );
    view.update(cx, |this, cx| {
        this.hosts_close(cx);
        this.open_hosts(cx);
        this.hosts_open_add(SheetMode::Edit, B, cx);
        // Editing the field does not change which host is deleted.
        this.setup_input.set_text("dev@elsewhere.local".into());
    });
    cx.run_until_parked();
    click(cx, "host-edit-delete");
    view.update(cx, |this, _| {
        let confirm = this.shell.hosts_sheet.delete.clone();
        assert_eq!(confirm.as_ref().map(|c| c.host.as_str()), Some(B));
        assert_eq!(confirm.map(|c| c.name), Some("Mini".to_owned()));
    });
    click(cx, "host-delete-confirm");
    view.update(cx, |this, _| {
        let sheet = &this.shell.hosts_sheet;
        assert!(sheet.add.is_none(), "the Edit sheet closes");
        assert!(sheet.list_open, "the list stays");
        assert!(sheet.delete.is_none());
    });
    // Host tests have no store: the job fails, and a failed delete keeps
    // the host listed with the reason.
    view.update(cx, |this, _| {
        assert!(!this.setup.busy);
        assert!(this.setup.hosts.contains(&B.to_owned()));
        assert!(
            this.setup.status.contains("unavailable"),
            "{}",
            this.setup.status
        );
    });
    view.update(cx, |this, _| {
        this.apply_setup_result(Ok(Reply::Deleted(
            SavedHosts {
                targets: vec![A.into(), C.into()],
                selected: A.into(),
                names: Default::default(),
            },
            B.into(),
        )));
        assert_eq!(this.setup.hosts, vec![A.to_owned(), C.to_owned()]);
        assert_eq!(this.setup.status, "Deleted Mini.");
    });
}

#[gpui::test]
fn deleting_the_connected_host_disconnects_it_and_selects_the_next(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A, B, C]);
    let shown = connect_shown(&view, cx, B);
    let (hidden, hidden_active) = hidden_peer();
    view.update(cx, |this, cx| {
        this.board.hidden.insert(C.into(), hidden);
        this.shell.herdr2.confirm_session(B, "space-1");
        this.hosts_ask_delete(B, cx);
        assert_eq!(
            this.shell.hosts_sheet.delete.as_ref().map(|c| c.connected),
            Some(true)
        );
        this.hosts_confirm_delete(cx);
        assert_eq!(this.terminal.reconnect.target, C, "the host after it");
        assert!(this.terminal.open);
        assert!(!this.board.hidden.contains_key(B));
        assert!(!this.board.hidden_core.contains_key(B));
        assert_eq!(this.shell.herdr2.selected_session(B), "default");
    });
    assert!(!shown.load(Ordering::Acquire), "B's connection was closed");
    assert!(
        hidden_active.load(Ordering::Acquire),
        "C's connection, now shown, stays"
    );
    view.update(cx, |this, _| {
        this.apply_setup_result(Ok(Reply::Deleted(
            SavedHosts {
                targets: vec![A.into(), C.into()],
                selected: C.into(),
                names: Default::default(),
            },
            B.into(),
        )));
        let record = this.launch_snapshot();
        let entries = record.selected.iter().chain(&record.others);
        assert!(entries.clone().all(|entry| entry.host != B), "{record:?}");
        assert!(entries.clone().any(|entry| entry.host == C));
    });
}

#[gpui::test]
fn deleting_the_last_shown_host_selects_the_one_before(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A, B, C]);
    let shown = connect_shown(&view, cx, C);
    view.update(cx, |this, cx| {
        this.hosts_ask_delete(C, cx);
        this.hosts_confirm_delete(cx);
        assert_eq!(this.terminal.reconnect.target, B);
    });
    assert!(!shown.load(Ordering::Acquire));
}

#[gpui::test]
fn deleting_a_hidden_host_keeps_the_shown_one(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A, B]);
    let shown = connect_shown(&view, cx, A);
    let (hidden, hidden_active) = hidden_peer();
    view.update(cx, |this, cx| {
        this.board.hidden.insert(B.into(), hidden);
        this.hosts_ask_delete(B, cx);
        assert_eq!(
            this.shell.hosts_sheet.delete.as_ref().map(|c| c.connected),
            Some(true)
        );
        this.hosts_confirm_delete(cx);
        assert_eq!(this.terminal.reconnect.target, A);
        assert!(this.terminal.open);
        assert!(this.board.hidden.is_empty());
    });
    assert!(shown.load(Ordering::Acquire), "A's connection stays");
    assert!(
        !hidden_active.load(Ordering::Acquire),
        "B's hidden connection closed"
    );
}

#[gpui::test]
fn deleting_the_only_host_returns_to_the_first_run_screen(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[B]);
    let shown = connect_shown(&view, cx, B);
    view.update(cx, |this, cx| {
        this.hosts_ask_delete(B, cx);
        this.hosts_confirm_delete(cx);
        assert!(!this.terminal.open, "no host is left to show");
        assert!(this.terminal.reconnect.target.is_empty());
    });
    assert!(!shown.load(Ordering::Acquire));
    view.update(cx, |this, cx| {
        this.apply_setup_result(Ok(Reply::Deleted(SavedHosts::default(), B.into())));
        assert!(this.setup.hosts.is_empty());
        assert!(!this.shell.hosts_sheet.visible());
        cx.notify();
    });
    cx.run_until_parked();
    // As at a first launch, the Add host sheet opens over the empty frame.
    view.update(cx, |this, _| {
        let mode = this.shell.hosts_sheet.add.as_ref().map(|s| s.mode);
        assert_eq!(mode, Some(SheetMode::Add));
    });
}

#[gpui::test]
fn delete_waits_for_a_running_job_and_ignores_an_unsaved_host(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, &[A]);
    view.update(cx, |this, cx| {
        this.hosts_ask_delete("dev@unsaved.local", cx);
        assert!(this.shell.hosts_sheet.delete.is_none());
        this.setup.busy = true;
        this.hosts_ask_delete(A, cx);
        assert!(this.shell.hosts_sheet.delete.is_none());
        this.setup.busy = false;
        this.hosts_ask_delete(A, cx);
        this.setup.busy = true;
        this.hosts_confirm_delete(cx);
        assert!(
            this.shell.hosts_sheet.delete.is_some(),
            "a confirmation answered while a job runs waits"
        );
    });
}
