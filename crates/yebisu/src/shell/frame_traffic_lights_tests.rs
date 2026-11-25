//! A Mac's traffic lights over the content's top row (herdr-uikit
//! `traffic_lights_over_content`, "Designed for iPad" with the content under
//! the hidden title bar). With the sidebar collapsed to the rail, the tab
//! row's first tab starts past them (the owner's report, 2026-11-25: the rail
//! put it under them). The full sidebar keeps the layout the owner accepted,
//! the compact header keeps clear, and an iPad, where the flag is false, draws
//! exactly as before.
use super::overlay_tests::terminal;
use crate::app::Smoke;
use gpui::{Bounds, Edges, Entity, Pixels, TestAppContext, VisualTestContext, px, size};

/// herdr-uikit's sample on a Mac (macOS 26.7.3, candidate build): the Mac's
/// 80 pt in UIKit points at its 0.77 scale on the left, nothing on the right.
const MAC: (f32, f32) = (103.92, 0.0);
/// iPadOS 26.5 window controls (`frame_controls_tests.rs`).
const IPAD: (f32, f32) = (66.0, 9.5);

/// Draws the frame with the sample `reach`, the traffic-lights flag, and the
/// sidebar as the rail or the full column.
fn draw(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    (left, right): (f32, f32),
    traffic_lights: bool,
    rail: bool,
) {
    view.update(cx, |this, cx| {
        this.shell.fit.window_controls = Some(Edges {
            left: px(left),
            right: px(right),
            ..Edges::default()
        });
        this.shell.fit.traffic_lights_over_content = traffic_lights;
        this.actions.board_hidden = rail;
        cx.notify();
    });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

/// The row's first item with the sidebar collapsed: on Herdr 1 the space
/// switcher, then the tabs (`render_tab_row`). On Herdr 2, the owner's case,
/// the first tab comes first; both are children of the same padded row.
const FIRST_ITEM: &str = "shell-space-switcher";

#[gpui::test]
fn a_collapsed_sidebar_keeps_the_first_tab_past_the_traffic_lights(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    draw(&view, cx, MAC, false, true);
    let under = bounds(cx, FIRST_ITEM);
    let under_tab = bounds(cx, "shell-tab-0");
    let row = bounds(cx, "shell-tab-row");
    assert!(
        under.left() < px(MAC.0),
        "positive control: without the flag the rail's tab row starts its first item at {:?}, \
         under the traffic lights",
        under.left()
    );
    assert_eq!(
        under.left(),
        row.left(),
        "positive control: the first item opens the row"
    );
    draw(&view, cx, MAC, true, true);
    let first = bounds(cx, FIRST_ITEM);
    let tab = bounds(cx, "shell-tab-0");
    assert!(
        first.left() >= px(MAC.0),
        "the row's first item must start past the traffic lights, at {:?}",
        first.left()
    );
    assert_eq!(
        tab.left() - under_tab.left(),
        first.left() - under.left(),
        "the tabs move with it"
    );
    assert_eq!(
        bounds(cx, "shell-tab-row"),
        row,
        "the row keeps its place and size; only its content moves"
    );
    assert_eq!(
        bounds(cx, "shell-new-tab").left(),
        tab.right(),
        "+ still follows the last tab"
    );
    assert_eq!(first.size, under.size, "the first item keeps its size");
    assert_eq!(tab.size, under_tab.size, "the tab keeps its size");
}

#[gpui::test]
fn the_full_sidebar_keeps_the_accepted_layout(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    draw(&view, cx, MAC, false, false);
    let selectors = ["shell-tab-0", "shell-tab-row", "shell-sidebar-toggle"];
    let without: Vec<_> = selectors.iter().map(|s| bounds(cx, s)).collect();
    draw(&view, cx, MAC, true, false);
    for (selector, before) in selectors.iter().zip(without) {
        assert_eq!(bounds(cx, selector), before, "{selector} must not move");
    }
    assert!(
        bounds(cx, "shell-sidebar-toggle").left() >= px(MAC.0),
        "positive control: the full sidebar's header already starts past the traffic lights"
    );
}

#[gpui::test]
fn an_ipad_rail_draws_its_tab_row_as_before(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    draw(&view, cx, (0.0, 0.0), false, true);
    let plain = bounds(cx, "shell-tab-0");
    draw(&view, cx, IPAD, false, true);
    assert_eq!(
        bounds(cx, "shell-tab-0"),
        plain,
        "iPadOS 26's window controls must not move the tab row (the flag is false on an iPad)"
    );
}

#[gpui::test]
fn the_compact_header_starts_past_the_traffic_lights(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    // A narrow Mac window, as in Split View: the compact header.
    cx.simulate_resize(size(px(320.0), px(834.0)));
    draw(&view, cx, (0.0, 0.0), false, false);
    let plain = bounds(cx, "shell-compact-menu");
    assert!(
        plain.left() < px(MAC.0),
        "positive control: without controls the menu sits at the edge"
    );
    draw(&view, cx, MAC, true, false);
    assert!(
        bounds(cx, "shell-compact-menu").left() >= px(MAC.0),
        "the compact header's first button must start past the traffic lights"
    );
}
