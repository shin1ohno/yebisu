use super::{Geometry, page_height};
use crate::layout::{Rect, TerminalFit, compact_window};

#[test]
fn compact_starts_below_the_rail_and_terminal_budget() {
    assert!(compact_window(320.0));
    assert!(compact_window(351.99));
    assert!(!compact_window(352.0));
    assert!(!compact_window(605.0));
}

#[test]
fn page_row_takes_44_points_only_with_multiple_panes() {
    let window = Geometry {
        width: 320.0,
        height: 834.0,
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };
    assert_eq!(page_height(0), 0.0);
    assert_eq!(page_height(1), 0.0);
    assert_eq!(page_height(2), 44.0);
    assert_eq!(page_height(3), 44.0);
    assert_eq!(
        window.fit(1, false),
        TerminalFit::Fits(Rect::new(0.0, 53.0, 320.0, 680.0)),
    );
    assert_eq!(
        window.fit(2, false),
        TerminalFit::Fits(Rect::new(0.0, 97.0, 320.0, 636.0)),
    );
}

#[gpui::test]
fn compact_header_draws_53_points_above_the_page_row_and_sidebar_overlay(
    cx: &mut gpui::TestAppContext,
) {
    use crate::{app::Smoke, mock_peer};
    use gpui::{AppContext, Context, Entity, Render, Window, div, px, size};
    use std::time::{Duration, Instant};

    struct CompactFrame(Entity<Smoke>);
    impl Render for CompactFrame {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::prelude::*;
            self.0.update(cx, |this, cx| {
                this.render_compact_frame(div().into_any_element(), window, cx)
            })
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| CompactFrame(cx.new(Smoke::new)));
    cx.simulate_resize(size(px(320.0), px(834.0)));
    let smoke = view.read_with(cx, |frame, _| frame.0.clone());
    let client = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    smoke.update(cx, |this, cx| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client.0));
        cx.notify();
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    while !smoke.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.focused_pane_count() == 2
    }) {
        assert!(Instant::now() < deadline, "two coherent panes");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    let top_bar = cx
        .debug_bounds("shell-compact-top-bar")
        .unwrap_or_else(|| panic!("top bar"));
    let pager = cx
        .debug_bounds("shell-pane-pager")
        .unwrap_or_else(|| panic!("page row"));
    assert_eq!(top_bar.size.height, px(52.0));
    assert_eq!(pager.origin.y - top_bar.origin.y, px(53.0));
    assert_eq!(pager.size.height, px(44.0));

    smoke.update(cx, |this, cx| {
        this.toggle_compact_panel(super::Panel::Sidebar);
        cx.notify();
    });
    cx.run_until_parked();
    let panel = cx
        .debug_bounds("shell-compact-sidebar")
        .unwrap_or_else(|| panic!("sidebar panel"));
    assert_eq!(panel.origin.y, pager.origin.y);
    assert_eq!(panel.size.width, px(300.0));
}

#[gpui::test]
fn compact_uses_13_points_only_when_no_font_size_is_saved(cx: &mut gpui::TestAppContext) {
    use gpui::{px, size};

    let (view, cx) = cx.add_window_view(|_, cx| crate::app::Smoke::new(cx));
    cx.simulate_resize(size(px(320.0), px(834.0)));
    view.update_in(cx, |this, window, _| {
        assert_eq!(this.terminal_font_metrics(window), (13.0, 18.0));
        assert_eq!(this.terminal.font.size, None);
        this.terminal.font.size = Some(20.0);
        assert_eq!(this.terminal_font_metrics(window).0, 20.0);
    });
    cx.simulate_resize(size(px(352.0), px(834.0)));
    view.update_in(cx, |this, window, _| {
        assert_eq!(this.terminal_font_metrics(window).0, 20.0);
        this.terminal.font.size = None;
        assert_eq!(this.terminal_font_metrics(window), (14.0, 20.0));
    });
}
