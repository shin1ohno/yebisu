//! A tapped result opens as its sidebar row would, from the identity it was
//! drawn with; the panel closes only after the target opened (issue AC16 to
//! AC20).
use super::*;
use crate::shell::{
    herdr2_frame::CoreStatus,
    search::{DismissReason, Restored, UNAVAILABLE, results::RowTarget},
    sidebar::view::SidebarTap,
};

fn select(fake: &RecordedSearch, cx: &mut VisualTestContext, revision: u64, id: u64) {
    push(fake, cx, SearchEvent::Selected { id, revision });
}

/// Waits until the board moved to `pane`, the input follows it and is
/// ready. Harness: the private peer's frame after a focus change does not
/// keep the size the view asked for, and the view asks again only once its
/// size is unknown (as `select_host_kind_inner` does, board.rs:279).
fn moved_to(view: &Entity<Smoke>, cx: &mut VisualTestContext, pane: &str) {
    wait(
        view,
        cx,
        "the board did not move to the selected row",
        |this| {
            this.refresh_terminal_input();
            let live = &this.terminal.live;
            this.terminal.navigating.is_none()
                && crate::board::snapshot(live)
                    .is_some_and(|s| s.focused_pane_id.as_deref() == Some(pane))
                && crate::terminal_input::destination(live, this.terminal.link.generation)
                    .is_some_and(|d| d.target == crate::terminal::InputTarget::Pane(pane.into()))
        },
    );
    view.update(cx, |this, cx| {
        this.terminal.size = None;
        cx.notify();
    });
    wait(
        view,
        cx,
        "the moved pane's input did not become ready",
        |this| {
            this.refresh_terminal_input();
            this.terminal.input.ready && this.terminal.size.is_some()
        },
    );
}

/// Selecting a row: navigation runs, the panel is asked to close with the
/// row's id, and the handle stays held until that dismissal arrives.
fn select_and_close(
    view: &Entity<Smoke>,
    fake: &RecordedSearch,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
    row: impl Fn(&crate::shell::search::SearchItem) -> bool,
    method: &str,
    pane: &str,
) {
    opened(view, fake, cx);
    let (revision, shown) = fake.shown();
    let id = shown
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .find(|item| row(item))
        .map(|item| item.id)
        .unwrap_or_else(|| panic!("fixture: the row is a result: {shown:?}"));
    select(fake, cx, revision, id);
    let sent_requests = requests(receipts);
    assert!(
        sent_requests.iter().any(|r| r.starts_with(method)),
        "no {method} request: {sent_requests:?}"
    );
    assert_eq!(fake.0.borrow().dismissed, [id]);
    assert!(
        !fake.dropped(),
        "the handle was released before the dismissal"
    );
    assert_eq!(phase(view, cx), Some(Phase::Closing));
    push(
        fake,
        cx,
        SearchEvent::Dismissed(DismissReason::Selected(id)),
    );
    assert!(fake.dropped(), "the handle outlived Dismissed(Selected)");
    push(fake, cx, SearchEvent::InputRestored(Restored::Text));
    moved_to(view, cx, pane);
}

/// A Herdr 1 space: the space opens, the panel closes, the filter stays, and
/// the next key reaches the pane the space focused, once.
#[gpui::test]
fn select_h1_space(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    view.update(cx, |this, _| this.set_sidebar_filter("second"));
    select_and_close(
        &view,
        &fake,
        &receipts,
        cx,
        |item| item.title == "second",
        "\"workspace.focus\"",
        "w2:p2",
    );
    assert_eq!(
        filter(&view, cx),
        "second",
        "the selection cleared the filter"
    );
    typed_once(&view, &receipts, cx, "w2:p2", "x");
}

/// A Herdr 1 agent on the same host, another pane: the row, the shown
/// target and the pane that receives the next key agree; the old pane
/// receives nothing (issue AC17).
#[gpui::test]
fn select_h1_agent_on_the_same_host(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    select_and_close(
        &view,
        &fake,
        &receipts,
        cx,
        |item| item.accessibility_label.contains("pane w2:p2"),
        "\"pane.focus\"",
        "w2:p2",
    );
    typed_once(&view, &receipts, cx, "w2:p2", "y");
}

/// A space of another host: that host's terminal takes the next key; the
/// host that was on screen receives nothing.
#[gpui::test]
fn select_h1_space_on_another_host(cx: &mut TestAppContext) {
    let (view, receipts_a, cx) = terminal_at(cx, "A", 1200.);
    let receipts_b = connect(&view, cx, "B");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let (revision, shown) = fake.shown();
    let id = shown
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .find(|i| i.title == "second" && i.detail.starts_with("A "))
        .map(|i| i.id)
        .unwrap_or_else(|| panic!("fixture: host A's space is a result: {shown:?}"));
    select(&fake, cx, revision, id);
    assert_eq!(fake.0.borrow().dismissed, [id], "the row did not open");
    close(&fake, cx, DismissReason::Selected(id), Restored::Text);
    wait(&view, cx, "host A is on screen", |this| {
        this.terminal.reconnect.target == "A"
    });
    moved_to(&view, cx, "w2:p2");
    type_text(&view, cx, "x");
    assert_eq!(sent(&view, &receipts_a, cx), ["w2:p2: TextCommit(\"x\")"]);
    assert!(
        sent(&view, &receipts_b, cx).is_empty(),
        "the old host received input"
    );
}

/// A Herdr 2 session row goes to `open_herdr2_row` with the identity it was
/// drawn with; a row drawn before a reconnect is refused and the panel stays.
#[gpui::test]
fn select_h2_space_and_agent(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let session = crate::core::Session::start(
        "search-fixture".into(),
        1,
        std::sync::Arc::new(std::sync::Mutex::new(crate::live::Live::default())),
        || Err(crate::Error::Ssh("fixture: never connects".into())),
        None,
        crate::shell::single_pane::Mailbox::detached(),
    )
    .unwrap_or_else(|e| panic!("session: {e}"));
    session.statuses_mailbox().publish(vec![CoreStatus {
        address: "t2".into(),
        status: serde_json::from_value(serde_json::json!({
            "app": "claude", "state": "idle", "title": "agent t2",
        }))
        .ok(),
    }]);
    view.update(cx, |this, cx| {
        this.terminal = crate::terminal_view::TerminalView {
            core_mode: true,
            core: Some(session),
            ..Default::default()
        };
        this.terminal.reconnect.target = "pro-dev".into();
        this.terminal.live.connected = true;
        this.terminal.open = true;
        cx.notify();
    });
    cx.run_until_parked();
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let (revision, shown) = fake.shown();
    let targets = view.read_with(cx, |this, _| {
        let rows = &this
            .shell
            .search
            .session
            .as_ref()
            .unwrap_or_else(|| panic!("session"))
            .rows;
        shown
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .filter_map(|i| rows.lookup(revision, i.id).cloned().map(|t| (i.id, t)))
            .collect::<Vec<_>>()
    });
    let space = targets
        .iter()
        .find(|(_, t)| {
            matches!(
                t,
                RowTarget::Herdr2 {
                    row: crate::shell::search::results::CoreTarget::Space(_),
                    ..
                }
            )
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("fixture: a Herdr 2 session row: {targets:?}"));
    assert!(
        targets.iter().any(|(_, t)| matches!(t, RowTarget::Herdr2 { row: crate::shell::search::results::CoreTarget::Agent(pane), host, .. } if pane == "t2" && host == "pro-dev")),
        "the Herdr 2 agent is not a result with its drawn identity: {targets:?}"
    );
    // Drawn before a reconnect: refused, the panel stays with a message.
    view.update(cx, |this, _| this.terminal.link.generation += 1);
    select(&fake, cx, revision, space);
    assert!(
        fake.0.borrow().dismissed.is_empty(),
        "a stale Herdr 2 row opened"
    );
    assert_eq!(fake.shown().1.message.as_deref(), Some(UNAVAILABLE));
    // Drawn on this connection: it opens and the panel is asked to close.
    view.update(cx, |this, _| this.terminal.link.generation -= 1);
    select(&fake, cx, revision, space);
    assert_eq!(fake.0.borrow().dismissed, [space]);
}

/// A row that can no longer open (its host reconnected, disconnected, or
/// its space is gone): the panel stays with the same query and a message,
/// and the terminal receives nothing.
#[gpui::test]
fn failed_open_keeps_panel_with_message(cx: &mut TestAppContext) {
    type Break = fn(&mut Smoke);
    let cases: [(&str, Break); 3] = [
        ("generation", |this| this.terminal.link.generation += 1),
        ("disconnected", |this| {
            if let Ok(mut live) = this.terminal.link.inbox.try_lock() {
                live.connected = false;
            }
        }),
        ("gone space", |this| {
            if let Some(session) = this.shell.search.session.as_mut() {
                let revision = 0;
                session.rows.replace(
                    revision,
                    vec![(
                        999,
                        RowTarget::Herdr1(SidebarTap::Space {
                            host: "A".into(),
                            id: "gone".into(),
                            boot: "boot-v1".into(),
                            generation: this.terminal.link.generation,
                        }),
                    )],
                );
            }
        }),
    ];
    for (name, broken) in cases {
        let (view, receipts, cx) = terminal(cx);
        let fake = install_fake(&view, cx);
        view.update(cx, |this, _| this.set_sidebar_filter("second"));
        opened(&view, &fake, cx);
        let (revision, id) = row_id(&fake, "second");
        let id = if name == "gone space" { 999 } else { id };
        let before = status(&view, cx);
        view.update(cx, |this, _| broken(this));
        select(&fake, cx, revision, id);
        assert!(
            fake.0.borrow().dismissed.is_empty(),
            "{name}: the panel closed"
        );
        assert!(!fake.dropped(), "{name}: the handle was released");
        assert_eq!(phase(&view, cx), Some(Phase::Open), "{name}");
        let (_, shown) = fake.shown();
        assert_eq!(shown.message.as_deref(), Some(UNAVAILABLE), "{name}");
        assert_eq!(filter(&view, cx), "second", "{name}: the query changed");
        assert_eq!(status(&view, cx), before, "{name}: the status line changed");
        assert!(
            sent(&view, &receipts, cx).is_empty(),
            "{name}: input was sent"
        );
        assert!(
            !requests(&receipts).iter().any(|r| r.contains("focus")),
            "{name}: a navigation was sent"
        );
    }
}

/// A tap on the list just replaced (UIKit had not shown the new one yet)
/// still opens its row.
#[gpui::test]
fn selection_from_previous_revision_opens(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let (revision, id) = row_id(&fake, "second");
    push(&fake, cx, SearchEvent::QueryChanged("sec".into()));
    assert_eq!(fake.shown().0, 1, "fixture: a newer revision is shown");
    select(&fake, cx, revision, id);
    assert_eq!(
        fake.0.borrow().dismissed,
        [id],
        "the previous list's row did not open"
    );
    assert!(
        requests(&receipts)
            .iter()
            .any(|r| r.contains("workspace.focus"))
    );
}

/// A row from neither the shown nor the previous list is ignored and logged.
#[gpui::test]
fn stale_row_ignored_and_logged(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    let (sender, records) = std::sync::mpsc::sync_channel(64);
    view.update(cx, |this, _| {
        this.observation = crate::observation::Observation::injected(sender);
    });
    opened(&view, &fake, cx);
    let (revision, id) = row_id(&fake, "second");
    push(&fake, cx, SearchEvent::QueryChanged("sec".into()));
    push(&fake, cx, SearchEvent::QueryChanged("seco".into()));
    select(&fake, cx, revision, id);
    assert!(fake.0.borrow().dismissed.is_empty(), "a stale row opened");
    assert!(!requests(&receipts).iter().any(|r| r.contains("focus")));
    let logged = records
        .try_iter()
        .any(|record| record["kind"] == "search-stale-row" && record["id"] == id);
    assert!(logged, "the stale row was not logged");
}

/// A failure known only after the navigation was sent keeps today's status
/// display and never presents the panel again.
#[gpui::test]
fn post_send_failure_does_not_reopen(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let (revision, id) = row_id(&fake, "second");
    select(&fake, cx, revision, id);
    close(&fake, cx, DismissReason::Selected(id), Restored::Text);
    view.update(cx, |this, cx| {
        this.terminal.live.status = "Navigation failed".into();
        cx.notify();
    });
    for _ in 0..5 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    assert_eq!(fake.presents(), 1, "the panel was presented again");
    assert!(phase(&view, cx).is_none());
}

/// Control for the selection tests: the same navigation from a plain
/// sidebar tap (no search) lets the next key reach the new pane once.
#[gpui::test]
fn control_sidebar_tap_then_key_reaches_the_pane(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    view.update(cx, |this, _| {
        let tap = SidebarTap::Space {
            host: "A".into(),
            id: "w2".into(),
            boot: "boot-v1".into(),
            generation: this.terminal.link.generation,
        };
        assert!(this.sidebar_tap(tap).is_ok(), "fixture: the tap navigates");
    });
    moved_to(&view, cx, "w2:p2");
    typed_once(&view, &receipts, cx, "w2:p2", "x");
}
