//! The Settings sheet keeps its header clear of iPadOS 26's window controls
//! when it sits at the top of a short window, with the reach the shell chrome
//! uses (`layout/controls.rs`), read through `platform_window_controls`
//! (shell/fit.rs) as the shell reads it. The UIKit sample itself is covered
//! by the Simulator check (docs/screenshots/w4-port/).
use crate::app::Smoke;
use gpui::{Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, px, size};

/// herdr-uikit's (left, right) samples on the iPadOS 26.5 Simulator (iPad
/// Pro 13-inch (M5)): controls on the left, on the right (right-to-left), a
/// full-screen corner allowance, and all zero (no controls).
const CONTROLS: (f32, f32) = (66.0, 9.5);
const CONTROLS_RIGHT: (f32, f32) = (9.0, 66.0);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);
const NONE: (f32, f32) = (0.0, 0.0);
/// The narrow window of the PR 87 check.
const NARROW: (f32, f32) = (411.0, 613.0);
/// Where the header's content starts without controls: the sheet's 16 pt
/// margin, its 1 pt border and the header's 8 pt padding.
const OWN_LEADING: f32 = 16.0 + 1.0 + 8.0;

fn sheet(
    cx: &mut TestAppContext,
    (width, height): (f32, f32),
    (left, right): (f32, f32),
) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(width), px(height)));
    view.update(cx, |this, _| {
        this.shell.fit.window_controls = Some(Edges {
            left: px(left),
            right: px(right),
            ..Edges::default()
        });
    });
    cx.update(|window, cx| view.update(cx, |this, cx| this.begin_settings_modal(window, cx)));
    cx.run_until_parked();
    (view, cx)
}

fn painted(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

#[gpui::test]
fn header_starts_past_window_controls_on_the_left(cx: &mut TestAppContext) {
    let (_view, cx) = sheet(cx, NARROW, CONTROLS);
    let panel = painted(cx, "settings-panel");
    assert_eq!((panel.origin.x, panel.origin.y), (px(16.), px(16.)));
    let state = painted(cx, "settings-save-state");
    assert_eq!(state.origin.x, px(CONTROLS.0 + 1.0));
    // The title and Done keep their places in the header.
    let close = painted(cx, "settings-close");
    assert!(close.origin.x > state.origin.x + state.size.width);
    assert_eq!(
        close.origin.x + close.size.width,
        px(NARROW.0 - 16.0 - 1.0 - 8.0)
    );
}

#[gpui::test]
fn done_ends_before_window_controls_on_the_right(cx: &mut TestAppContext) {
    let (_view, cx) = sheet(cx, NARROW, CONTROLS_RIGHT);
    let close = painted(cx, "settings-close");
    assert_eq!(
        close.origin.x + close.size.width,
        px(NARROW.0 - CONTROLS_RIGHT.1 - 1.0)
    );
    assert_eq!(painted(cx, "settings-save-state").origin.x, px(OWN_LEADING));
}

#[gpui::test]
fn no_controls_or_a_corner_allowance_moves_nothing(cx: &mut TestAppContext) {
    for reach in [NONE, FULL_SCREEN] {
        let (_view, cx) = sheet(cx, NARROW, reach);
        assert_eq!(
            painted(cx, "settings-save-state").origin.x,
            px(OWN_LEADING),
            "{reach:?}"
        );
        let close = painted(cx, "settings-close");
        assert_eq!(
            close.origin.x + close.size.width,
            px(NARROW.0 - OWN_LEADING)
        );
    }
}

#[gpui::test]
fn a_sheet_below_the_controls_band_moves_nothing(cx: &mut TestAppContext) {
    // Tall enough for the canvas position, 136 pt down.
    let (_view, cx) = sheet(cx, (411.0, 1000.0), CONTROLS);
    assert_eq!(painted(cx, "settings-panel").origin.y, px(136.));
    assert_eq!(painted(cx, "settings-save-state").origin.x, px(OWN_LEADING));
}
