//! Terminal input around a session: the discard fence and the revision
//! rule, no native retirement while the panel holds first responder, and
//! one clean-up for every way a session ends (issue AC12, AC23).
use super::*;
use crate::shell::search::{DismissReason, FailReason, Restored};

fn fenced(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |this, _| this.terminal.input.requires_gesture())
}

/// Opens over a discarded composition (the fence is up), then closes with
/// `restored`; `between` runs while the panel is up.
fn open_over_composition(
    view: &Entity<Smoke>,
    fake: &RecordedSearch,
    cx: &mut VisualTestContext,
    restored: Restored,
    between: impl FnOnce(&mut Smoke),
) {
    mark_text(view, cx, "にほ");
    opened(view, fake, cx);
    assert!(
        fenced(view, cx),
        "fixture: the open's detach fenced the composition"
    );
    view.update(cx, |this, _| between(this));
    close(fake, cx, DismissReason::Return, restored);
    assert!(phase(view, cx).is_none());
}

/// The text view took first responder back and nothing detached in
/// between: the fence lifts and the next key reaches the terminal once.
#[gpui::test]
fn unchanged_revision_releases_the_fence(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    open_over_composition(&view, &fake, cx, Restored::Text, |_| {});
    assert!(
        !fenced(&view, cx),
        "the fence stayed after InputRestored(Text)"
    );
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}

/// A detach while the panel was up (a host switch, say) changed the
/// revision: the fence stays until a terminal gesture.
#[gpui::test]
fn changed_revision_keeps_fence(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    open_over_composition(&view, &fake, cx, Restored::Text, |this| {
        this.terminal.input.detach();
    });
    assert!(fenced(&view, cx), "a changed revision released the fence");
    type_text(&view, cx, "q");
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "a key passed the fence"
    );
    view.update(cx, |this, _| this.terminal.input.fresh_gesture());
    typed_once(&view, &receipts, cx, "w1:p1", "z");
}

/// Drawing, None and WindowClosed restores never lift the fence.
#[gpui::test]
fn non_text_restore_keeps_fence(cx: &mut TestAppContext) {
    for restored in [Restored::Drawing, Restored::None, Restored::WindowClosed] {
        let (view, _receipts, cx) = terminal(cx);
        let fake = install_fake(&view, cx);
        open_over_composition(&view, &fake, cx, restored, |_| {});
        assert!(fenced(&view, cx), "{restored:?} released the fence");
    }
}

/// While the panel holds first responder, a sidebar transition (a host tap)
/// submits no native text retirement; after the restore the next one does.
#[gpui::test]
fn no_retirement_while_open(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    mark_text(&view, cx, "にほ");
    opened(&view, &fake, cx);
    view.update(cx, |this, _| {
        this.retirement.reason = None;
        assert!(
            this.terminal.input.requires_gesture(),
            "fixture: a fence to retire"
        );
        let _ = this.sidebar_tap(crate::shell::sidebar::view::SidebarTap::Host("A".into()));
        assert_eq!(
            this.retirement.reason, None,
            "a retirement was requested while the panel was up"
        );
    });
    close(&fake, cx, DismissReason::Cancel, Restored::Drawing);
    view.update(cx, |this, _| {
        let _ = this.sidebar_tap(crate::shell::sidebar::view::SidebarTap::Host("A".into()));
        assert!(
            this.retirement.reason.is_some(),
            "fixture: after the restore a transition requests retirement"
        );
    });
}

/// `Failed` drops the handle at once and waits for the SDK's restore; the
/// restore ends the session once, and a late duplicate changes nothing.
#[gpui::test]
fn failed_then_restored_cleans_once(cx: &mut TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    tap_magnifier(cx);
    assert_eq!(fake.presents(), 1, "fixture: presented");
    push(&fake, cx, SearchEvent::Failed(FailReason::PresenterBusy));
    assert!(fake.dropped(), "the handle was kept after Failed");
    assert_eq!(phase(&view, cx), Some(Phase::Restoring));
    assert_eq!(status(&view, cx), "Search could not open (PresenterBusy)");
    push(&fake, cx, SearchEvent::InputRestored(Restored::Text));
    assert!(
        phase(&view, cx).is_none(),
        "the session outlived the restore"
    );
    typed_once(&view, &receipts, cx, "w1:p1", "x");
    // The next session starts clean, and the magnifier opens it.
    opened(&view, &fake, cx);
}

fn ends_once(reason: DismissReason, restored: Restored, cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx);
    let refreshed = view.read_with(cx, |this, _| this.terminal.input.retirement_revision());
    push(&fake, cx, SearchEvent::Dismissed(reason));
    assert!(fake.dropped(), "{reason:?}: the handle was kept");
    assert_eq!(
        phase(&view, cx),
        Some(Phase::Restoring),
        "{reason:?}: the session ended before the restore"
    );
    push(&fake, cx, SearchEvent::InputRestored(restored));
    assert!(
        phase(&view, cx).is_none(),
        "{reason:?}: the session was kept"
    );
    // An event after the end belongs to no session: nothing changes.
    let after = view.read_with(cx, |this, _| this.terminal.input.retirement_revision());
    view.update_in(cx, |this, window, cx| {
        this.on_search_event(1, SearchEvent::InputRestored(restored), window, cx);
    });
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.input.retirement_revision()),
        after,
        "{reason:?}: a second clean-up ran"
    );
    assert!(after >= refreshed);
    assert_eq!(fake.presents(), 1);
}

#[gpui::test]
fn window_closed_cleans_once(cx: &mut TestAppContext) {
    ends_once(DismissReason::WindowClosed, Restored::WindowClosed, cx);
}

#[gpui::test]
fn dismissed_system_cleans_once(cx: &mut TestAppContext) {
    ends_once(DismissReason::System, Restored::Text, cx);
}
