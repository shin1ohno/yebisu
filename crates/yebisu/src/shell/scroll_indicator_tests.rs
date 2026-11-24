//! Issue #137 PR 5 (C1): the list scroll thumb.
use super::*;
use crate::{app::Smoke, mock_peer, settings_pages::Page};
use gpui::{
    Modifiers, ScrollDelta, ScrollEvent, ScrollSource, TestAppContext, TouchPhase,
    VisualTestContext,
};
use std::time::Duration;

/// T25: content that fits shows no thumb.
#[test]
fn content_that_fits_shows_no_thumb() {
    assert_eq!(thumb_geometry(300., 0., 0.), None);
    assert_eq!(thumb_geometry(300., -4., 0.), None);
    assert_eq!(thumb_geometry(0., 100., 0.), None);
    assert_eq!(thumb_geometry(f32::NAN, 100., 0.), None);
}

/// T25: the thumb is as long as the share of the content in view, at least
/// 36 pt, and moves along the track inside 2 pt insets.
#[test]
fn the_thumb_is_at_least_36_pt_and_follows_the_offset() {
    let geometry = |viewport, max, scrolled| {
        thumb_geometry(viewport, max, scrolled).unwrap_or_else(|| panic!("overflowing"))
    };
    // Half the content in view: half the height.
    assert_eq!(geometry(300., 300., 0.), (2., 150.));
    // A long list: the shortest thumb, at the top, middle and bottom.
    assert_eq!(geometry(100., 10_000., 0.), (2., 36.));
    assert_eq!(geometry(100., 10_000., 5_000.), (32., 36.));
    assert_eq!(geometry(100., 10_000., 10_000.), (62., 36.));
    // Past either end (rubber-banding is off, but a stale offset may be).
    assert_eq!(geometry(100., 10_000., 20_000.), (62., 36.));
    assert_eq!(geometry(100., 10_000., -50.), (2., 36.));
}

/// T26: the thumb shows while the list moves and for 1.0 s after.
#[test]
fn the_thumb_shows_for_one_second_after_the_last_move() {
    let moved = Instant::now();
    assert!(visible(moved, moved));
    assert!(visible(moved, moved + Duration::from_millis(900)));
    assert!(!visible(moved, moved + Duration::from_millis(1100)));
}

/// T27: every list the issue names tracks its own indicator and paints its
/// thumb as its last child, so the thumb draws over its rows. Rendered
/// checks for representative lists follow.
#[test]
fn every_list_tracks_its_indicator_and_draws_its_thumb() {
    let sources = [
        ("sidebar", include_str!("frame.rs")),
        ("rail", include_str!("rail/view.rs")),
        (
            "settings_categories",
            include_str!("../settings_sheet/view.rs"),
        ),
        (
            "settings_content",
            include_str!("../settings_sheet/view.rs"),
        ),
        ("first_run", include_str!("hosts_sheet/view.rs")),
        ("hosts_list", include_str!("hosts_sheet/view.rs")),
        ("add_host", include_str!("hosts_sheet/view.rs")),
        ("nearby", include_str!("hosts_sheet/nearby_view.rs")),
        ("shortcuts", include_str!("shortcuts/view.rs")),
        ("overflow_menu", include_str!("overflow_menu.rs")),
        ("new_space", include_str!("herdr2_frame/new_space.rs")),
        ("compact_sidebar", include_str!("compact/panels.rs")),
        ("compact_tabs", include_str!("compact/panels.rs")),
    ];
    for (field, source) in sources {
        let flat: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        let tracked =
            format!(".overflow_y_scroll().track_scroll(self.shell.indicators.{field}.handle())");
        let thumb = format!("self.shell.indicators.{field}.thumb()");
        assert_eq!(
            flat.matches(&tracked).count(),
            1,
            "{field} tracks its handle"
        );
        assert_eq!(flat.matches(&thumb).count(), 1, "{field} paints its thumb");
    }
}

fn settings(cx: &mut TestAppContext) -> (gpui::Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(640.)));
    let (client, _) = mock_peer::connect_quiet().unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("scroll-indicator");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.begin_settings_modal(window, cx);
            this.settings_page = Page::Input;
            cx.notify();
        })
    });
    cx.run_until_parked();
    (view, cx)
}

fn redraw(view: &gpui::Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

fn swipe(cx: &mut VisualTestContext, at: Point<Pixels>, y: f32) {
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), px(y))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Touch,
    });
    cx.run_until_parked();
}

/// T26, T27: the overflowing Settings Input page shows no thumb until it is
/// scrolled, then one at the list's right edge, gone a second later.
#[gpui::test]
fn a_scrolled_settings_page_shows_its_thumb_for_a_second(cx: &mut TestAppContext) {
    let (view, cx) = settings(cx);
    let indicator = view.read_with(cx, |this, _| this.shell.indicators.settings_content.clone());
    let list = indicator.handle().bounds();
    assert!(list.size.height > px(0.), "the content tracks its handle");
    assert!(
        indicator.handle().max_offset().y > px(0.),
        "the Input page overflows at this height"
    );
    redraw(&view, cx);
    assert_eq!(indicator.painted(), None, "no thumb before a scroll");
    swipe(cx, list.center(), -120.);
    redraw(&view, cx);
    let Some(thumb) = indicator.painted() else {
        panic!("a scroll shows the thumb");
    };
    let list = indicator.handle().bounds();
    assert_eq!(thumb.right(), list.right() - px(2.));
    assert_eq!(thumb.size.width, px(3.));
    assert!(
        thumb.top() > list.top() && thumb.bottom() < list.bottom(),
        "{thumb:?} {list:?}"
    );
    assert!(thumb.size.height >= px(36.));
    std::thread::sleep(Duration::from_millis(1100));
    redraw(&view, cx);
    assert_eq!(indicator.painted(), None, "gone after a second at rest");
}

/// T25 rendered: a list whose content fits never shows a thumb, even
/// under a swipe.
#[gpui::test]
fn a_list_that_fits_never_shows_a_thumb(cx: &mut TestAppContext) {
    let (view, cx) = settings(cx);
    // Tall enough for every category.
    cx.simulate_resize(size(px(1200.), px(1600.)));
    redraw(&view, cx);
    let indicator = view.read_with(cx, |this, _| {
        this.shell.indicators.settings_categories.clone()
    });
    let list = indicator.handle().bounds();
    assert!(
        list.size.height > px(0.),
        "the categories track their handle"
    );
    if indicator.handle().max_offset().y > px(0.) {
        panic!("the categories overflow at this height; the fixture needs a taller window");
    }
    swipe(cx, list.center(), -120.);
    redraw(&view, cx);
    assert_eq!(indicator.painted(), None);
}
