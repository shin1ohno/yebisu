use super::Geometry;
use crate::{
    app::Smoke,
    layout::{Rect, TerminalFit, compact_window},
};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};

#[test]
fn compact_starts_below_the_rail_and_terminal_budget() {
    assert!(compact_window(320.0));
    assert!(compact_window(351.99));
    assert!(!compact_window(352.0));
    assert!(!compact_window(605.0));
}

#[test]
fn page_row_takes_44_points_only_when_visible() {
    let window = Geometry {
        width: 320.0,
        height: 834.0,
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };
    assert_eq!(
        window.fit(false, false),
        TerminalFit::Fits(Rect::new(0.0, 53.0, 320.0, 680.0)),
    );
    assert_eq!(
        window.fit(true, false),
        TerminalFit::Fits(Rect::new(0.0, 97.0, 320.0, 636.0)),
    );
}

pub(super) fn assert_terminal_origin(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    pager_height: f32,
) {
    cx.run_until_parked();
    let top = cx
        .debug_bounds("shell-compact-top-bar")
        .unwrap_or_else(|| panic!("top bar"))
        .origin;
    let pager = cx.debug_bounds("shell-pane-pager");
    assert_eq!(
        pager.map_or(px(0.0), |row| row.size.height),
        px(pager_height)
    );
    let terminal = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("terminal"));
    assert_eq!(terminal.origin.y - top.y, px(53.0 + pager_height));
    let fit = view.update_in(cx, |this, window, _| {
        this.compact_terminal_fit(window)
            .and_then(|fit| fit.rect())
            .unwrap_or_else(|| panic!("terminal fit"))
    });
    assert_eq!(terminal.origin, gpui::point(px(fit.x), px(fit.y)));
}

#[gpui::test]
fn compact_header_draws_53_points_above_the_page_row_and_sidebar_overlay(cx: &mut TestAppContext) {
    use crate::mock_peer;
    use std::time::{Duration, Instant};

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(320.0), px(834.0)));
    let client = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client.0));
        cx.notify();
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.pager().is_some()
    }) {
        assert!(Instant::now() < deadline, "two coherent panes");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_terminal_origin(&view, cx, 44.0);
    let top_bar = cx
        .debug_bounds("shell-compact-top-bar")
        .unwrap_or_else(|| panic!("top bar"));
    let pager = cx
        .debug_bounds("shell-pane-pager")
        .unwrap_or_else(|| panic!("page row"));
    assert_eq!(top_bar.size.height, px(52.0));
    assert_eq!(pager.origin.y - top_bar.origin.y, px(53.0));
    assert_eq!(pager.size.height, px(44.0));

    view.update(cx, |this, cx| {
        this.toggle_compact_panel(super::Panel::Sidebar);
        cx.notify();
    });
    cx.run_until_parked();
    let panel = cx
        .debug_bounds("shell-compact-sidebar")
        .unwrap_or_else(|| panic!("sidebar panel"));
    assert_eq!(panel.origin.y, pager.origin.y);
    assert_eq!(panel.size.width, px(300.0));

    view.update(cx, |this, cx| {
        this.toggle_compact_panel(super::Panel::Sidebar);
        this.terminal.live.connected = false;
        assert!(this.terminal.coherent_frame().is_some());
        cx.notify();
    });
    assert_terminal_origin(&view, cx, 0.0);
    let original = view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        let surface = this
            .terminal
            .live
            .surface
            .as_mut()
            .unwrap_or_else(|| panic!("surface"));
        let original = surface.clone();
        let surface = std::sync::Arc::make_mut(surface);
        surface.popup = Some(Box::new(sheprd_client::protocol::ClientShellPopupSurface {
            terminal_id: "popup".into(),
            title: "Test popup".into(),
            width: None,
            height: None,
            frame: surface.frame.clone(),
            mouse_reporting: false,
            sgr_pixel_mouse: false,
            pixel_width: 0,
            pixel_height: 0,
        }));
        assert!(this.terminal.coherent_frame().is_some());
        cx.notify();
        original
    });
    assert_terminal_origin(&view, cx, 0.0);
    view.update(cx, |this, cx| {
        this.terminal.live.surface = Some(original);
        this.settings_modal = true;
        cx.notify();
    });
    assert_terminal_origin(&view, cx, 0.0);
    view.update(cx, |this, cx| {
        this.settings_modal = false;
        cx.notify();
    });
    assert_terminal_origin(&view, cx, 44.0);

    cx.simulate_resize(size(px(352.0), px(834.0)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("shell-compact-frame").is_none());
    let terminal = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("rail terminal"));
    assert_eq!(terminal.origin.x, px(72.0));
    assert_eq!(terminal.size.width, px(280.0));
}

#[gpui::test]
fn compact_uses_13_points_only_when_no_font_size_is_saved(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
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

#[gpui::test]
fn compact_frame_and_font_use_width_inside_both_safe_area_edges(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        this.shell.fit.insets = Some(gpui::WindowInsets {
            safe_area: gpui::Edges {
                left: px(12.0),
                right: px(8.0),
                ..gpui::Edges::default()
            },
            ..gpui::WindowInsets::default()
        });
        cx.notify();
    });
    cx.simulate_resize(size(px(360.0), px(834.0)));
    assert_terminal_origin(&view, cx, 0.0);
    assert!(cx.debug_bounds("shell-compact-frame").is_some());
    let terminal = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("terminal"));
    assert_eq!(terminal.origin.x, px(12.0));
    assert_eq!(terminal.size.width, px(340.0));
    view.update_in(cx, |this, window, _| {
        assert_eq!(this.terminal_font_metrics(window), (13.0, 18.0));
    });

    cx.simulate_resize(size(px(372.0), px(834.0)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("shell-compact-frame").is_none());
    let terminal = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("rail terminal"));
    assert_eq!(terminal.origin.x, px(84.0));
    assert_eq!(terminal.size.width, px(280.0));

    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("shell-too-small").is_some());
    assert!(
        cx.debug_bounds("shell-compact-frame").is_some(),
        "short compact window keeps its frame"
    );
}

#[gpui::test]
fn compact_settings_first_larger_click_and_qa_use_the_terminal_default(cx: &mut TestAppContext) {
    use crate::settings::SettingsHost;
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(format!("yebisu-compact-font-{}", std::process::id()));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(320.0), px(834.0)));
    view.update_in(cx, |this, window, cx| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        this.settings_fonts = vec![this.terminal.font.family.clone()];
        this.begin_settings_modal(window, cx);
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until, "settings loaded");
        std::thread::sleep(Duration::from_millis(2));
    }
    let plus = cx
        .debug_bounds("font-plus")
        .unwrap_or_else(|| panic!("Larger"));
    let qa_font = |this: &mut Smoke, window: &mut gpui::Window, _: &mut gpui::Context<Smoke>| {
        this.qa_ax_fields(window)
            .into_iter()
            .find_map(|(key, value)| (key == "font_size").then_some(value))
    };
    assert_eq!(view.update_in(cx, qa_font), Some(serde_json::json!(13.0)));
    cx.simulate_click(plus.center(), Default::default());
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.font.size),
        Some(14.0)
    );
    assert_eq!(view.update_in(cx, qa_font), Some(serde_json::json!(14.0)));
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until, "settings saved");
        std::thread::sleep(Duration::from_millis(2));
    }
    std::fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
