//! iPadOS 26's window controls over the window's top-left corner (L0
//! `layout/controls.rs`): the painted sidebar keeps its header content and
//! the rail's first item clear of them, and a full-screen window's corner
//! allowance moves nothing.
use super::overlay_tests::terminal;
use crate::{
    app::Smoke,
    layout::{ControlsClearance, SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING},
    shell::rail::RAIL_HEADER_HEIGHT,
};
use gpui::{Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, px};

/// UIKit's reach (left, right) on the iPadOS 26.5 Simulator (iPad Pro
/// 13-inch (M5), the app's `window_controls` record): a window with the
/// controls showing, and the corner allowance alone in a full-screen window.
const CONTROLS: (f32, f32) = (66.0, 9.5);
const FULL_SCREEN: (f32, f32) = (9.5, 9.0);

/// Draws the frame with the controls reaching `reach`, the sidebar as the
/// rail or the full column, and returns where `selector` was painted.
fn painted(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    reach: (f32, f32),
    rail: bool,
    selector: &'static str,
) -> Bounds<Pixels> {
    view.update(cx, |this, cx| {
        this.shell.fit.window_controls = Some(Edges {
            left: px(reach.0),
            right: px(reach.1),
            ..Edges::default()
        });
        this.actions.board_hidden = rail;
        cx.notify();
    });
    cx.run_until_parked();
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted with the controls at {reach:?}"))
}

#[gpui::test]
fn rail_first_item_starts_under_the_window_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let none = painted(&view, cx, (0.0, 0.0), true, "rail-item-0");
    assert_eq!(
        none.origin.y,
        px(RAIL_HEADER_HEIGHT),
        "positive control: without controls the list starts under the 8 pt header"
    );
    let full_screen = painted(&view, cx, FULL_SCREEN, true, "rail-item-0");
    assert_eq!(
        full_screen.origin, none.origin,
        "a full-screen window's corner allowance must not move the rail"
    );
    let controls = painted(&view, cx, CONTROLS, true, "rail-item-0");
    assert_eq!(
        controls.origin.y,
        px(SIDEBAR_HEADER_HEIGHT),
        "the first item must start under the header band the controls sit in, \
         where the full sidebar's list starts"
    );
    let recorded = view.read_with(cx, |this, _| this.shell.fit.recorded_controls);
    assert_eq!(
        recorded,
        Some((
            CONTROLS.0,
            CONTROLS.1,
            ControlsClearance {
                header_leading: CONTROLS.0,
                rail_top: SIDEBAR_HEADER_HEIGHT,
            },
        )),
        "the observation log must get UIKit's reach and the clearance"
    );
}

#[gpui::test]
fn full_sidebar_header_starts_past_the_window_controls(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    let field = "sidebar-filter-unavailable";
    let none = painted(&view, cx, (0.0, 0.0), false, field);
    assert_eq!(
        none.origin.x,
        px(SIDEBAR_HEADER_PADDING),
        "positive control: without controls the header keeps its 12 pt padding"
    );
    let full_screen = painted(&view, cx, FULL_SCREEN, false, field);
    assert_eq!(
        full_screen.origin, none.origin,
        "a full-screen window's corner allowance must not move the header"
    );
    let controls = painted(&view, cx, CONTROLS, false, field);
    assert_eq!(
        controls.origin.x,
        px(CONTROLS.0),
        "the header's content must start past the controls"
    );
    assert_eq!(
        controls.origin.y, none.origin.y,
        "only the header's content moves; the header keeps its place and height"
    );
}
