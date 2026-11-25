//! The name shown while the host confirms it, and how it ends (issue AC11,
//! AC13): success and a newer snapshot, an error or rejection, the 10 s
//! timeout, a disconnect; and the field closing when its target goes.
use super::*;
use crate::{
    board::snapshot,
    mock_peer,
    shell::{
        rename::{CANCELLED, UNAVAILABLE_HERE, send::NOT_CONFIRMED},
        style::color,
    },
};

/// A Herdr 1 terminal on a peer with `fault` (`rename-error`, `rename-silent`).
fn faulty<'a>(
    cx: &'a mut gpui::TestAppContext,
    fault: &'static str,
) -> (Entity<Smoke>, Receipts, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(gpui::size(gpui::px(1200.), gpui::px(1000.)));
    let (client, receipts) =
        mock_peer::connect_recorded_fault(fault).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait(&view, cx, "private peer did not become ready", |this| {
        this.terminal.live.surface.is_some() && snapshot(&this.terminal.live).is_some()
    });
    (view, receipts, cx)
}

fn label(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    object: &RenameObject,
) -> Option<(String, u32)> {
    view.read_with(cx, |this, _| this.rename_label("boot-v1", object))
}

#[gpui::test]
fn pending_label_at_eight_sites(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-silent");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    assert_eq!(
        label(&view, cx, &space("w1")),
        Some(("ship".into(), color::MUTED)),
        "the sent name is drawn muted (sidebar row, switcher, tab row)"
    );
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.menu_space_name().as_deref(),
            Some("Space 1 · ship"),
            "the Space menu's name"
        );
        let mut items = crate::shell::rail::rail_items(&this.rail_hosts(), &this.setup);
        this.rail_names(&mut items);
        assert!(
            items.iter().any(|item| item.name == "ship"),
            "the rail item and its menu header"
        );
        assert_eq!(
            this.shown_name("boot-v1", &space("w1"), "repo".into()),
            "ship",
            "the space switcher and list"
        );
        // The snapshot itself is never rewritten.
        let shown = snapshot(&this.terminal.live).unwrap_or_else(|| panic!("snapshot"));
        assert_eq!(shown.workspaces[0].label, "repo");
    });
}

#[gpui::test]
fn pane_chip_takes_the_pending_name(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-silent");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, pane("w1:p1"), RenameAnchor::PaneChip);
    commit(&fake, cx, "agent");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    view.read_with(cx, |this, _| {
        let chip = this.shell.rename.chip("boot-v1");
        assert_eq!(
            chip.pending_for("w1:p1"),
            Some(("agent".into(), color::MUTED))
        );
    });
}

#[gpui::test]
fn success_then_snapshot(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    let sent_revision = view.read_with(cx, |this, _| {
        snapshot(&this.terminal.live).map_or(0, |s| s.revision)
    });
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    wait(&view, cx, "the host's snapshot did not name it", |this| {
        this.shell.rename.pending.is_none()
            && snapshot(&this.terminal.live)
                .is_some_and(|s| s.revision > sent_revision && s.workspaces[0].label == "ship")
    });
    assert_eq!(
        label(&view, cx, &space("w1")),
        None,
        "the snapshot's name shows"
    );
    assert_ne!(status(&view, cx), NOT_CONFIRMED);
}

#[gpui::test]
fn error_restores_and_reports(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-error");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    wait(
        &view,
        cx,
        "the error did not end the pending name",
        |this| this.shell.rename.pending.is_none(),
    );
    assert_eq!(status(&view, cx), "Rename failed: workspace not found");
    assert_eq!(
        label(&view, cx, &space("w1")),
        None,
        "the old name shows again"
    );
    view.read_with(cx, |this, _| {
        assert_eq!(this.terminal.live.pending_request, None);
        assert!(this.terminal.live.connected);
    });
}

#[gpui::test]
fn command_rejected(cx: &mut gpui::TestAppContext) {
    let mut ticket = Some(crate::shell::rename::RenameTicket {
        request: "r1".into(),
        outcome: None,
    });
    let theirs = crate::shell::rename::RenameTicket::reject(&mut ticket, Some("r2"), "busy");
    assert!(!theirs, "another request's rejection is not the rename's");
    assert_eq!(ticket.as_ref().and_then(|t| t.outcome.clone()), None);
    let ours = crate::shell::rename::RenameTicket::reject(&mut ticket, Some("r1"), "busy");
    assert!(ours);
    assert_eq!(ticket.and_then(|t| t.outcome), Some(Err("busy".to_owned())));
    let _ = cx;
}

#[gpui::test]
fn timeout_10s(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-silent");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    cx.executor().advance_clock(Duration::from_secs(9));
    cx.run_until_parked();
    assert!(
        label(&view, cx, &space("w1")).is_some(),
        "still waiting before 10 s"
    );
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(label(&view, cx, &space("w1")), None);
    assert_eq!(status(&view, cx), NOT_CONFIRMED);
}

#[gpui::test]
fn disconnect_clears(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-silent");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    view.update(cx, |this, cx| {
        this.terminal.live.connected = false;
        cx.notify();
    });
    draw(&view, cx);
    assert_eq!(label(&view, cx, &space("w1")), None);
}

#[gpui::test]
fn boot_change_clears(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = faulty(cx, "rename-silent");
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    view.update(cx, |this, cx| {
        if let Some(pending) = this.shell.rename.pending.as_mut() {
            pending.boot = "old-boot".into();
        }
        cx.notify();
    });
    draw(&view, cx);
    view.read_with(cx, |this, _| assert!(this.shell.rename.pending.is_none()));
}

#[gpui::test]
fn status_left_unchanged(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = faulty(cx, "rename-error");
    let fake = install_fake(&view, cx);
    let left = view.read_with(cx, |this, _| {
        (this.terminal.live.connected, this.chrome_state().connected)
    });
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    wait(&view, cx, "the error did not arrive", |this| {
        this.shell.rename.pending.is_none()
    });
    let after = view.read_with(cx, |this, _| {
        (this.terminal.live.connected, this.chrome_state().connected)
    });
    assert_eq!(
        left, after,
        "the status bar's left side (connection) is unchanged"
    );
}

/// The target leaves the snapshot while the field is up: it closes with
/// the object's text.
#[gpui::test]
fn target_removed_closes(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w2"), RenameAnchor::SpaceRow);
    view.update(cx, |this, cx| {
        let mut shown = snapshot(&this.terminal.live)
            .cloned()
            .unwrap_or_else(|| panic!("snapshot"));
        shown.workspaces.retain(|w| w.workspace_id != "w2");
        this.terminal.live.snapshot = Some(std::sync::Arc::new(shown));
        this.terminal.live.pending_snapshot = None;
        cx.notify();
    });
    draw(&view, cx);
    assert_eq!(fake.dismissed(), 1, "the field was not closed");
    assert_eq!(status(&view, cx), "This space was closed.");
}

/// The anchor stops being drawn (the sidebar collapsed): two frames
/// without it close the field with "Rename cancelled".
#[gpui::test]
fn anchor_missing_twice_cancels(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    view.update(cx, |this, cx| {
        this.toggle_sidebar();
        cx.notify();
    });
    draw(&view, cx);
    assert_eq!(fake.dismissed(), 1, "the field stayed without its anchor");
    assert_eq!(status(&view, cx), CANCELLED);
}

/// An anchor never drawn: the rename ends before the SDK is asked.
#[gpui::test]
fn never_drawn_unavailable(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    view.update(cx, |this, cx| {
        this.toggle_sidebar();
        cx.notify();
    });
    draw(&view, cx);
    begin(&view, cx, space("w1"), RenameAnchor::SpaceRow);
    draw(&view, cx);
    assert_eq!(fake.presents(), 0, "presented over an anchor never drawn");
    assert_eq!(phase(&view, cx), None);
    assert_eq!(status(&view, cx), UNAVAILABLE_HERE);
}

/// Accepted but not yet in a snapshot: the name stays, in the normal color,
/// until a snapshot newer than the one it was sent at.
#[gpui::test]
fn confirmed_until_a_newer_snapshot(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.update(cx, |this, _| {
        let shown = snapshot(&this.terminal.live)
            .cloned()
            .unwrap_or_else(|| panic!("snapshot"));
        this.shell.rename.pending = Some(crate::shell::rename::PendingLabel {
            boot: shown.boot_id.clone(),
            object: space("w1"),
            text: "ship".into(),
            sent_revision: shown.revision,
            request: "r1".into(),
            state: crate::shell::rename::send::PendingState::Sending,
            sent_at: std::time::Instant::now(),
        });
        this.terminal.live.rename = Some(crate::shell::rename::RenameTicket {
            request: "r1".into(),
            outcome: Some(Ok(())),
        });
        this.settle_pending_rename();
        assert_eq!(
            this.rename_label(&shown.boot_id, &space("w1")),
            Some(("ship".into(), color::TEXT)),
            "accepted: the name stays, no longer muted"
        );
        let mut newer = shown.clone();
        newer.revision += 1;
        this.terminal.live.snapshot = Some(std::sync::Arc::new(newer));
        this.terminal.live.pending_snapshot = None;
        this.settle_pending_rename();
        assert!(
            this.shell.rename.pending.is_none(),
            "a newer snapshot names it"
        );
    });
}
