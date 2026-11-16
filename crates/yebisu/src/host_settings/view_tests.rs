//! Real sheet route and click adapter; no SSH or platform credential operations.
use crate::{app::Smoke, settings::SettingsHost, theme::Theme, theme_picker::Palette};
use gpui::{TestAppContext, px, size};
use std::time::{Duration, Instant};

#[gpui::test]
fn appearance_page_routes_to_real_theme_handler_and_current_host_status(cx: &mut TestAppContext) {
    let root = std::env::var_os("P15_FIXTURE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/p15-tests")
        })
        .join(format!("route-{}", std::process::id()));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1100.), px(900.)));
    view.update(cx, |this, _| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
    let open = cx
        .debug_bounds("open-settings")
        .unwrap_or_else(|| panic!("settings entry"));
    cx.simulate_click(open.center(), Default::default());
    cx.run_until_parked();
    let page = cx
        .debug_bounds("settings-page-appearance")
        .unwrap_or_else(|| panic!("appearance route"));
    cx.simulate_click(page.center(), Default::default());
    cx.run_until_parked();
    let palette = cx
        .debug_bounds("high-contrast")
        .unwrap_or_else(|| panic!("theme button"));
    cx.simulate_click(palette.center(), Default::default());
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        assert_eq!(this.local_theme, Palette::HighContrast);
        assert_eq!(Theme::from_palette(this.local_theme).background, 0);
        this.settings_lifecycle(gpui::AppLifecyclePhase::Background);
        this.poll_settings(cx);
    });
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    let bytes = std::fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}"));
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(json["theme"], "high-contrast");
    assert_eq!(json["drag_select"], true);
    assert_eq!(json["copy_on_select"], false);
    for (target, connected) in [
        ("fixture-a", true),
        ("fixture-b", true),
        ("fixture-b", false),
    ] {
        view.update(cx, |this, cx| {
            this.terminal.reconnect.target = target.into();
            this.terminal.live.connected = connected;
            assert_eq!(
                this.host_settings_status(),
                (
                    target.to_owned(),
                    crate::host_settings_availability::current(connected)
                )
            );
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("host-settings-status").is_some());
    }
    std::fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
