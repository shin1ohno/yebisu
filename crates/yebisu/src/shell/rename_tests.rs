//! Rename in place (issue #143): host tests with a recorded field on the
//! private Herdr 1 peer.
//!
//! Every test that guards a path while the field is up first calls
//! `opened`: the anchor was drawn, the fake presented exactly once and the
//! session is `Open` after `Opened`. A regression that never opens fails
//! there, not at the guarded step. `scripts/ios/rename-mutations.py` removes
//! each guarded line in turn and requires the named test to fail.
use super::{FieldDismiss, FieldEvent, FieldRestored, Phase, RenameAnchor, RenameObject};
use crate::app::Smoke;
pub(crate) use crate::shell::search::tests::{Receipts, sent, terminal, wait};
use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, VisualTestContext, point,
};
use std::time::Duration;

#[path = "rename_tests/confirm.rs"]
mod confirm;
#[path = "rename_tests/entry.rs"]
mod entry;
#[path = "rename_tests/fake.rs"]
pub(crate) mod fake;
#[path = "rename_tests/herdr2.rs"]
mod herdr2;
#[path = "rename_tests/isolation.rs"]
mod isolation;
#[path = "rename_tests/normalize.rs"]
mod normalize;
#[path = "rename_tests/send.rs"]
mod send;
#[path = "rename_tests/terminal.rs"]
mod terminal;

pub(crate) use fake::RecordedField;

pub(crate) fn install_fake(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> RecordedField {
    let fake = RecordedField::default();
    let presenter = fake.clone();
    view.update(cx, |this, _| {
        this.shell.rename.set_presenter(Box::new(presenter));
    });
    fake
}

/// A few frames, each one drawing the window.
pub(crate) fn draw(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    for _ in 0..4 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
}

/// Starts a rename of `object` over `anchor` on the selected host.
pub(crate) fn begin(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    object: RenameObject,
    anchor: RenameAnchor,
) {
    view.update(cx, |this, _| {
        let target = this
            .rename_target(object)
            .unwrap_or_else(|| panic!("fixture: no rename target"));
        this.begin_rename(target, anchor);
    });
    draw(view, cx);
}

pub(crate) fn push(fake: &RecordedField, cx: &mut VisualTestContext, event: FieldEvent) {
    fake.push(event);
    cx.run_until_parked();
}

pub(crate) fn phase(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<Phase> {
    view.read_with(cx, |this, _| {
        this.shell.rename.session.as_ref().map(|s| s.phase)
    })
}

pub(crate) fn status(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.terminal.live.status.clone())
}

/// The precondition of every guarded step: the anchor was drawn, the field
/// presented once over it, and the session is `Open`.
pub(crate) fn opened(
    view: &Entity<Smoke>,
    fake: &RecordedField,
    cx: &mut VisualTestContext,
    object: RenameObject,
    anchor: RenameAnchor,
) {
    let before = fake.presents();
    begin(view, cx, object, anchor);
    assert_eq!(
        fake.presents(),
        before + 1,
        "the field was not presented over {anchor:?} (status {:?})",
        status(view, cx)
    );
    assert_eq!(phase(view, cx), Some(Phase::Presenting));
    push(fake, cx, FieldEvent::Opened);
    assert_eq!(
        phase(view, cx),
        Some(Phase::Open),
        "the session is not open"
    );
}

/// Closes the field the way the SDK does: the dismissal, then the restore.
pub(crate) fn close(
    fake: &RecordedField,
    cx: &mut VisualTestContext,
    reason: FieldDismiss,
    restored: FieldRestored,
) {
    push(fake, cx, FieldEvent::Dismissed(reason));
    push(fake, cx, FieldEvent::InputRestored(restored));
}

/// Commits `text` and closes, as Return does.
pub(crate) fn commit(fake: &RecordedField, cx: &mut VisualTestContext, text: &str) {
    push(fake, cx, FieldEvent::Committed(text.into()));
    close(fake, cx, FieldDismiss::Committed, FieldRestored::Text);
}

/// The rename requests the peer received since the last call, as
/// `method params` strings (params' keys sorted).
pub(crate) fn renames(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
) -> Vec<String> {
    for _ in 0..5 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut seen = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let sheprd_client::protocol::ClientMessage::ClientShellEndpointRequest {
            request, ..
        } = message
        {
            let value: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|e| panic!("request: {e}"));
            let method = value["method"].as_str().unwrap_or_default().to_owned();
            if method.ends_with(".rename") {
                seen.push(format!("{method} {}", value["params"]));
            }
        }
    }
    seen
}

/// Every endpoint request method the peer received since the last call.
pub(crate) fn methods(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
) -> Vec<String> {
    for _ in 0..5 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut seen = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let sheprd_client::protocol::ClientMessage::ClientShellEndpointRequest {
            request, ..
        } = message
        {
            let value: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|e| panic!("request: {e}"));
            seen.push(value["method"].as_str().unwrap_or_default().to_owned());
        }
    }
    seen
}

/// Two taps at the middle of `selector`, as GPUI's touch synthesizes them.
pub(crate) fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not painted"));
    let position = point(
        at.origin.x + at.size.width / 2.,
        at.origin.y + at.size.height / 2.,
    );
    for click_count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count,
        });
        cx.run_until_parked();
    }
}

pub(crate) fn space(id: &str) -> RenameObject {
    RenameObject::Space(id.into())
}
pub(crate) fn tab(id: &str) -> RenameObject {
    RenameObject::Tab(id.into())
}
pub(crate) fn pane(id: &str) -> RenameObject {
    RenameObject::Pane(id.into())
}
