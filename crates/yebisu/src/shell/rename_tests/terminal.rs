//! The terminal around a rename (issue AC5): what it was composing is
//! dropped before the field opens, nothing reaches it while the field is up,
//! the next key after the restore arrives once, and the SDK handle is
//! released only after the commit was sent.
use super::*;
use crate::shell::search::tests::{mark_text, type_text, typed_once};

#[gpui::test]
fn detach_before_open(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    sent(&view, &receipts, cx);
    mark_text(&view, cx, "にほ");
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    view.read_with(cx, |this, _| {
        assert!(
            this.terminal.input.composition.text.is_empty(),
            "the composition survived the open"
        );
    });
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "the composition reached the peer"
    );
}

#[gpui::test]
fn nothing_reaches_peer_while_open(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    sent(&view, &receipts, cx);
    // On the host there is no SDK gate (the probe in herdr-uikit proves it
    // keeps GPUI's input handler empty); what yebisu owns is the detach's
    // fence over the composition it dropped.
    mark_text(&view, cx, "にほ");
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    type_text(&view, cx, "x");
    mark_text(&view, cx, "ぷ");
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "input reached the peer while the field was up"
    );
    push(&fake, cx, FieldEvent::Dismissed(FieldDismiss::Escape));
    type_text(&view, cx, "y");
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "input reached the peer before the restore"
    );
}

#[gpui::test]
fn first_key_after_restore_once(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    sent(&view, &receipts, cx);
    mark_text(&view, cx, "にほ");
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    close(&fake, cx, FieldDismiss::Escape, FieldRestored::Text);
    assert_eq!(phase(&view, cx), None);
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}

#[gpui::test]
fn order_committed_dismissed_restored(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    push(&fake, cx, FieldEvent::Committed("ship".into()));
    assert!(!fake.dropped(), "the handle went before Dismissed");
    assert_eq!(phase(&view, cx), Some(Phase::Open));
    push(&fake, cx, FieldEvent::Dismissed(FieldDismiss::Committed));
    assert!(fake.dropped(), "the handle was kept after Dismissed");
    assert_eq!(phase(&view, cx), Some(Phase::Restoring));
    push(&fake, cx, FieldEvent::InputRestored(FieldRestored::Text));
    assert_eq!(phase(&view, cx), None);
    assert_eq!(renames(&view, &receipts, cx).len(), 1);
}

#[gpui::test]
fn drop_after_send(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    push(&fake, cx, FieldEvent::Committed("ship".into()));
    // The request was handed to the client before the handle went.
    let sent_before_drop = view.read_with(cx, |this, _| this.shell.rename.pending.is_some());
    push(&fake, cx, FieldEvent::Dismissed(FieldDismiss::Committed));
    assert!(
        sent_before_drop && fake.dropped(),
        "the handle went before the send"
    );
    assert_eq!(renames(&view, &receipts, cx).len(), 1);
}

/// Issue #143 widened #138's gate: while the rename field holds first
/// responder, a transition submits no native text retirement (the host
/// path, `input_retirement.rs`); after the restore the next one does.
#[gpui::test]
fn no_retirement_while_rename_open(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    mark_text(&view, cx, "にほ");
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    view.update(cx, |this, _| {
        this.retirement.reason = None;
        assert!(
            this.terminal.input.requires_gesture(),
            "fixture: a fence to retire"
        );
        let _ = this.sidebar_tap(crate::shell::sidebar::view::SidebarTap::Host("A".into()));
        assert_eq!(
            this.retirement.reason, None,
            "a retirement was requested while the rename field was up"
        );
    });
    close(&fake, cx, FieldDismiss::Escape, FieldRestored::Drawing);
    view.update(cx, |this, _| {
        let _ = this.sidebar_tap(crate::shell::sidebar::view::SidebarTap::Host("A".into()));
        assert!(
            this.retirement.reason.is_some(),
            "fixture: after the restore a transition requests retirement"
        );
    });
}

/// The key band (none while the field owns the keyboard) and its presses.
#[gpui::test]
fn key_band_gone_while_open(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    view.read_with(cx, |this, _| {
        assert!(this.native_field_open());
        let ime = gpui::Edges::default();
        assert_eq!(
            this.key_band_now(crate::layout::Size::new(800., 1000.), &ime),
            0.0
        );
    });
}
