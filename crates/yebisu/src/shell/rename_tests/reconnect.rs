//! The field across a reconnect (#270, #143 D13): going to another app and
//! back pauses the connection and dials a new one. The same boot keeps the
//! field and its text, and a commit goes to the new connection; another boot
//! or a missing object closes it; and after such a close nothing typed
//! reaches the terminal until it is tapped.
use super::*;
use crate::{
    board::snapshot,
    mock_peer,
    shell::search::tests::{type_text, typed_once},
};
use gpui::{AppLifecyclePhase, point, px};

/// Home: the app goes to the background (the connection pauses). Manual
/// reconnect, so the return does not dial a real host.
fn away(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, cx| {
        this.terminal.reconnect.automatic = false;
        this.lifecycle(AppLifecyclePhase::Background, cx);
    });
    draw(view, cx);
}

/// Back: the app is active again and a new connection (a new generation)
/// reaches the private peer, whose boot is `boot-v1` again.
fn back(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Receipts {
    view.update(cx, |this, cx| this.lifecycle(AppLifecyclePhase::Active, cx));
    draw(view, cx);
    let (client, receipts) = mock_peer::connect_recorded(1).unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait(
        view,
        cx,
        "the new connection did not become ready",
        |this| this.terminal.live.surface.is_some() && snapshot(&this.terminal.live).is_some(),
    );
    draw(view, cx);
    sent(view, &receipts, cx);
    receipts
}

/// The field, still the one presented first, has not been asked to close.
fn kept(view: &Entity<Smoke>, fake: &RecordedField, cx: &mut VisualTestContext, when: &str) {
    assert_eq!(
        (fake.dismissed(), fake.presents(), phase(view, cx)),
        (0, 1, Some(Phase::Open)),
        "{when}: the field did not stay open with its text (status {:?})",
        status(view, cx)
    );
}

/// The boot the field was opened on is not the one the host runs now.
fn restarted(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, _| {
        if let Some(session) = this.shell.rename.session.as_mut() {
            session.target.boot = "boot-before-restart".into();
        }
    });
}

/// A tap on the terminal, as a finger's press and release on one cell.
fn tap_terminal(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let view = view.clone();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            let g = this.terminal.geometry;
            let at = g.grid.origin + point(px(1.5 * g.cell_width), px(1.5 * g.cell_height));
            this.selection_down(
                &MouseDownEvent {
                    position: at,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                },
                window,
                cx,
            );
            this.selection_up(window, cx);
        });
    });
    cx.run_until_parked();
}

fn terminal_status(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.terminal_status())
}

#[gpui::test]
fn same_boot_reconnect_keeps_field(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    away(&view, cx);
    kept(&view, &fake, cx, "in the background");
    let receipts = back(&view, cx);
    kept(&view, &fake, cx, "after the same-boot reconnect");
    view.read_with(cx, |this, _| {
        let target = this.shell.rename.session.as_ref().map(|s| s.target.clone());
        assert_eq!(
            target.map(|t| (t.generation, t.boot)),
            Some((this.terminal.link.generation, "boot-v1".into())),
            "the target did not move to the new connection of its boot"
        );
    });
    assert!(
        renames(&view, &receipts, cx).is_empty(),
        "the reconnect sent a rename"
    );
}

#[gpui::test]
fn commit_after_same_boot_reconnect(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w2"), RenameAnchor::SpaceRow);
    away(&view, cx);
    let receipts = back(&view, cx);
    // The field held "ab" before home and took "cd" after the return.
    commit(&fake, cx, "abcd");
    assert_eq!(
        renames(&view, &receipts, cx),
        [r#"workspace.rename {"workspace_id":"w2","label":"abcd"}"#],
        "the commit did not reach the new connection for the space (status {:?})",
        status(&view, cx)
    );
}

#[gpui::test]
fn commit_while_away_refused(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    away(&view, cx);
    kept(&view, &fake, cx, "in the background");
    commit(&fake, cx, "abcd");
    assert_eq!(
        status(&view, cx),
        "Rename failed: SSH operation failed: Not connected"
    );
    let receipts = back(&view, cx);
    assert!(
        renames(&view, &receipts, cx).is_empty(),
        "a commit made while away was sent after the return"
    );
}

#[gpui::test]
fn commit_refused_when_target_gone(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    away(&view, cx);
    kept(&view, &fake, cx, "in the background");
    // The space is not in the new connection's snapshot.
    view.update(cx, |this, _| {
        if let Some(session) = this.shell.rename.session.as_mut() {
            session.target.object = space("w-closed");
        }
    });
    let receipts = back(&view, cx);
    commit(&fake, cx, "abcd");
    assert!(
        renames(&view, &receipts, cx).is_empty(),
        "a rename was sent for a space that is gone"
    );
    assert_eq!(
        status(&view, cx),
        "Rename failed: SSH operation failed: This space was closed."
    );
}

#[gpui::test]
fn boot_change_closes_field(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    away(&view, cx);
    kept(&view, &fake, cx, "in the background");
    restarted(&view, cx);
    let receipts = back(&view, cx);
    assert_eq!(fake.dismissed(), 1, "a boot change kept the field");
    view.read_with(cx, |this, _| {
        let moved = this
            .shell
            .rename
            .session
            .as_ref()
            .map(|s| s.target.generation == this.terminal.link.generation);
        assert_eq!(
            moved,
            Some(false),
            "the target moved to a connection of another boot"
        );
    });
    close(&fake, cx, FieldDismiss::Requested, FieldRestored::Text);
    // The connection's own status replaced the text; the hold keeps it.
    assert_eq!(
        terminal_status(&view, cx),
        "This space was closed. · tap terminal to type"
    );
    assert!(renames(&view, &receipts, cx).is_empty());
}

/// The #270 report: the field closed during a reconnect, then `cd` reached
/// the shell. After a close for a boot change nothing typed reaches the
/// terminal until it is tapped; after the tap a key arrives once.
#[gpui::test]
fn typing_after_close_waits_for_tap(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    away(&view, cx);
    restarted(&view, cx);
    let receipts = back(&view, cx);
    if phase(&view, cx).is_some() {
        close(&fake, cx, FieldDismiss::Requested, FieldRestored::Text);
    }
    assert_eq!(phase(&view, cx), None, "fixture: the field closed");
    type_text(&view, cx, "c");
    type_text(&view, cx, "d");
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "text typed after the close reached the terminal"
    );
    assert!(
        terminal_status(&view, cx).ends_with("tap terminal to type"),
        "the status does not ask for a tap: {:?}",
        terminal_status(&view, cx)
    );
    tap_terminal(&view, cx);
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}

/// The same hold after the object went while connected (no reconnect).
#[gpui::test]
fn typing_after_target_removed_waits_for_tap(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w2"), RenameAnchor::SpaceRow);
    view.update(cx, |this, _| {
        if let Some(session) = this.shell.rename.session.as_mut() {
            session.target.object = space("w-closed");
        }
    });
    draw(&view, cx);
    assert_eq!(fake.dismissed(), 1, "fixture: the field was asked to close");
    close(&fake, cx, FieldDismiss::Requested, FieldRestored::Text);
    sent(&view, &receipts, cx);
    type_text(&view, cx, "cd");
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "text typed after the close reached the terminal"
    );
    tap_terminal(&view, cx);
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}
