//! The key band while the panel is up: none in the normal frame and for the
//! compact pager; back after the restore (issue AC9, AC15). The band keys
//! themselves are guarded in `key_band::tests` (they reach the private
//! press path).
use super::*;
use crate::shell::search::{DismissReason, Restored};

/// A docked software keyboard: the pager-only band above it (#229).
fn software_keyboard(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, cx| {
        this.shell.fit.insets = Some(gpui::WindowInsets {
            ime: gpui::Edges {
                bottom: px(498.),
                ..gpui::Edges::default()
            },
            ..gpui::WindowInsets::default()
        });
        this.shell.fit.hardware_keyboard_attached = false;
        this.shell.fit.keyboard_placement = crate::shell::key_band::bar::Placement::Docked;
        cx.notify();
    });
    cx.run_until_parked();
}

fn band(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    let view = view.clone();
    cx.update(|window, cx| view.read(cx).frame_layout(window).key_band.is_some())
}

fn pager(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    let view = view.clone();
    cx.update(|window, cx| view.read(cx).pager_available_at(window))
}

#[gpui::test]
fn band_zero_while_open(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal_at(cx, "A", 1376.);
    let fake = install_fake(&view, cx);
    software_keyboard(&view, cx);
    assert!(
        band(&view, cx),
        "fixture: a docked software keyboard has the pager band"
    );
    assert!(cx.debug_bounds("shell-pager-band").is_some());
    opened(&view, &fake, cx);
    assert!(!band(&view, cx), "the band was kept while the panel is up");
    assert!(cx.debug_bounds("shell-pager-band").is_none());
    push(&fake, cx, SearchEvent::Dismissed(DismissReason::Return));
    assert!(!band(&view, cx), "the band came back before the restore");
    push(&fake, cx, SearchEvent::InputRestored(Restored::Text));
    assert!(
        band(&view, cx),
        "the band did not come back after the restore"
    );
}

#[gpui::test]
fn compact_pager_band_zero(cx: &mut TestAppContext) {
    let (view, _receipts, cx) = terminal_at(cx, "A", 1376.);
    let fake = install_fake(&view, cx);
    software_keyboard(&view, cx);
    assert!(
        pager(&view, cx),
        "fixture: the band makes the pager available"
    );
    opened(&view, &fake, cx);
    assert!(
        !pager(&view, cx),
        "the pager still counts the band while the panel is up"
    );
    close(&fake, cx, DismissReason::Cancel, Restored::Text);
    assert!(pager(&view, cx));
}
