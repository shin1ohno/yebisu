//! Integration 1 review fa5a: the frame's floating layers (L5 edit bar, L2a
//! ... menu) stay inside their own region and keep presses to themselves.
use crate::{app::Smoke, handoff::Selection, mock_peer};
use gpui::{
    Bounds, Entity, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext, point, px,
    size,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// A Herdr 1 terminal on the private peer, drawn in the frame at 1200x1000.
pub(super) fn terminal(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (client, _receipts) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("frame-overlay");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx)
}

fn selection(view: &Smoke) -> Selection {
    // Polling drops a selection whose cells do not match the current surface.
    // The quiet peer keeps this displayed frame stable across the press.
    let surface = view
        .terminal
        .live
        .surface
        .as_ref()
        .unwrap_or_else(|| panic!("fixture surface"));
    Selection {
        frame: Arc::new(surface.frame.clone()),
        anchor: (1, 2),
        end: (6, 2),
        dragging: false,
    }
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not painted"))
}

fn overlaps(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}

fn redraw(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

#[gpui::test]
fn edit_bar_floats_over_the_terminal_not_the_tab_row(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| this.handoff.selection = Some(selection(this)));
    redraw(&view, cx);
    assert!(view.read_with(cx, |this, _| this.is_selecting()));
    let bar = bounds(cx, "edit-bar");
    let tab = bounds(cx, "shell-tab-body-0");
    let terminal = bounds(cx, "shell-terminal");
    assert!(
        !overlaps(bar, tab),
        "edit bar {bar:?} covers the tab {tab:?}"
    );
    assert!(
        bar.origin.y >= terminal.origin.y && bar.origin.x >= terminal.origin.x,
        "edit bar {bar:?} starts outside the terminal slot {terminal:?}"
    );
    assert!(
        bar.origin.x + bar.size.width <= terminal.origin.x + terminal.size.width,
        "edit bar {bar:?} is wider than the terminal slot {terminal:?}"
    );
}

#[gpui::test]
fn overflow_menu_press_never_reaches_the_terminal(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| {
        this.handoff.selection = Some(selection(this));
        this.shell.frame.overflow_open = true;
    });
    redraw(&view, cx);
    let menu = bounds(cx, "shell-overflow-menu");
    let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
    // A point of the menu over the terminal grid, where terminal-input's
    // mouse down starts a selection (terminal_view.rs).
    let at = point(
        menu.origin.x + menu.size.width / 2.,
        grid.origin.y + px(10.),
    );
    assert!(
        menu.contains(&at) && grid.contains(&at),
        "fixture: menu {menu:?} must lie over grid {grid:?}"
    );
    // The terminal acts on mouse down (selection_down); the item's own action
    // runs on click, so the press is held to observe only the terminal.
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let kept = view.read_with(cx, |this, _| {
        this.handoff.selection.as_ref().map(|s| (s.anchor, s.end))
    });
    assert_eq!(
        kept,
        Some(((1, 2), (6, 2))),
        "the press replaced the selection"
    );
}

#[gpui::test]
fn overflow_menu_closes_on_an_outside_press_that_stays_out_of_the_terminal(
    cx: &mut TestAppContext,
) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| {
        this.handoff.selection = Some(selection(this));
        this.shell.frame.overflow_open = true;
    });
    redraw(&view, cx);
    assert!(view.read_with(cx, |this, _| this.is_selecting()));
    let menu = bounds(cx, "shell-overflow-menu");
    let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
    // On the terminal grid, left of the menu.
    let at = point(grid.origin.x + px(20.), grid.origin.y + px(10.));
    assert!(
        !menu.contains(&at) && grid.contains(&at),
        "fixture: the press must be on the grid outside the menu"
    );
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let (open, kept) = view.read_with(cx, |this, _| {
        (
            this.shell.frame.overflow_open,
            this.handoff.selection.as_ref().map(|s| (s.anchor, s.end)),
        )
    });
    assert!(!open, "an outside press closes the menu");
    assert_eq!(
        kept,
        Some(((1, 2), (6, 2))),
        "the closing press started a selection"
    );
    redraw(&view, cx);
    assert!(cx.debug_bounds("shell-overflow-backdrop").is_none());
}

#[gpui::test]
fn herdr2_without_a_sidebar_list_keeps_the_footer_column(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    let (collapsed, drawn, herdr1) = view.update_in(cx, |this, window, cx| {
        let layout = this.frame_layout(window);
        let herdr1 = this.render_sidebar_column(&layout, cx).is_some();
        this.terminal.core_mode = true;
        let drawn = this.render_sidebar_column(&layout, cx).is_some();
        (layout.sidebar_collapsed, drawn, herdr1)
    });
    assert!(!collapsed, "fixture: 1200 pt keeps the sidebar open");
    assert!(herdr1, "fixture: Herdr 1 draws its sidebar column");
    assert!(
        drawn,
        "Herdr 2 with no sidebar list must still draw the column that holds the footer"
    );
    let footer: Vec<_> = crate::shell::footer::footer_items(crate::shell::HerdrKind::Herdr2)
        .into_iter()
        .map(|entry| (entry.item, entry.enabled))
        .collect();
    use crate::shell::footer::FooterItem;
    assert!(
        footer.contains(&(FooterItem::Settings, true))
            && footer.contains(&(FooterItem::Hosts, true)),
        "Herdr 2's footer reaches Settings and Hosts: {footer:?}"
    );
}

#[gpui::test]
fn rail_mode_tab_row_shows_the_space_switcher(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    assert!(cx.debug_bounds("shell-space-switcher").is_none());
    view.update(cx, |this, _| this.actions.board_hidden = true);
    redraw(&view, cx);
    let switcher = bounds(cx, "shell-space-switcher");
    let tab = bounds(cx, "shell-tab-body-0");
    assert!(switcher.right() <= tab.left());
    assert!(switcher.size.width <= px(180.));
    cx.simulate_click(switcher.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("shell-space-picker").is_some());
}

#[gpui::test]
fn mods_item_has_icon_count_badge_and_label(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    cx.simulate_resize(size(px(605.), px(834.)));
    view.update(cx, |this, _| {
        this.shell.fit.mod_regions = crate::layout::Regions::maximum();
    });
    redraw(&view, cx);
    let button = bounds(cx, "shell-status-mods");
    let badge = bounds(cx, "shell-status-mods-count");
    assert_eq!(badge.size.height, px(16.));
    assert!(badge.size.width >= px(16.));
    assert!(button.contains(&badge.center()));
    assert_eq!(
        crate::shell::mods::mods_accessibility_label(&[crate::layout::Region::M2]).as_deref(),
        Some("Mods, 1 withdrawn: right dock")
    );
}

#[gpui::test]
fn drawn_new_tab_follows_the_last_tab_and_stays_left_of_split(cx: &mut TestAppContext) {
    let (_view, cx) = terminal(cx);
    let tab = bounds(cx, "shell-tab-0");
    let plus = bounds(cx, "shell-new-tab");
    let split = bounds(cx, "shell-split");
    assert_eq!(plus.left(), tab.right());
    assert!(split.left() > plus.right());
}

#[gpui::test]
fn drawn_overflow_menu_uses_its_column_insets(cx: &mut TestAppContext) {
    let (view, cx) = terminal(cx);
    view.update(cx, |this, _| this.shell.frame.overflow_open = true);
    redraw(&view, cx);
    let row = bounds(cx, "shell-tab-row");
    let menu = bounds(cx, "shell-overflow-menu");
    assert_eq!(menu.left() + menu.size.width, row.right() - px(8.));
    assert_eq!(menu.top(), row.top() + px(40.));
    assert_eq!(menu.size.width, px(260.));
}
