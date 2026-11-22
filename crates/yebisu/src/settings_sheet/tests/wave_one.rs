use crate::{
    app::Smoke,
    settings::{SettingsHost, sidebar_density::SidebarDensity},
};
use gpui::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext, point, px, size};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn open_sheet(
    cx: &mut TestAppContext,
    settings: Option<SettingsHost>,
) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.settings = settings;
            this.settings_fonts = vec!["SauceCodePro Nerd Font Mono".into()];
            this.begin_settings_modal(window, cx);
        })
    });
    cx.run_until_parked();
    (view, cx)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"))
}

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    let center = bounds(cx, selector).center();
    cx.simulate_click(center, Default::default());
    cx.run_until_parked();
}

fn store(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("wave-one-{name}-{}", std::process::id()))
}

fn wait_for_save(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        let failure = view.read_with(cx, |this, _| this.settings_error.clone());
        assert_eq!(failure, None, "settings worker failed");
        assert!(Instant::now() < until, "settings did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
}

fn document(root: &Path) -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[gpui::test]
fn settings_header_is_saved_title_done(cx: &mut TestAppContext) {
    let root = store("header");
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("{error}"));
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
    let (view, cx) = open_sheet(cx, Some(host));
    wait_for_save(&view, cx);

    let header = bounds(cx, "settings-header");
    let saved = bounds(cx, "settings-save-state");
    let title = bounds(cx, "settings-title");
    let done = bounds(cx, "settings-close");
    assert_eq!(header.size.height, px(60.));
    assert_eq!(saved.size.width, px(72.));
    assert_eq!(done.size, size(px(72.), px(44.)));
    assert!(saved.center().x < title.center().x && title.center().x < done.center().x);
    assert_eq!(title.center().x, header.center().x);
    assert!(cx.debug_bounds("settings-save-failure").is_none());
    press(cx, "settings-close");
    assert!(!view.read_with(cx, |this, _| this.settings_modal));
    drop(view);
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("{error}"));
}

#[gpui::test]
fn two_way_choices_are_segments(cx: &mut TestAppContext) {
    let (_, cx) = open_sheet(cx, None);
    press(cx, "settings-page-input");
    let native = bounds(cx, "settings-option-key-0");
    let alt = bounds(cx, "settings-option-key-1");
    assert_eq!(native.size.height, px(40.));
    assert_eq!(alt.size.height, px(40.));
    assert_eq!(native.origin.y, alt.origin.y);
    assert_eq!(native.right(), alt.left());
    let drag = bounds(cx, "settings-start-selection-1");
    let double_tap = bounds(cx, "settings-start-selection-0");
    assert_eq!(drag.right(), double_tap.left());
}

#[gpui::test]
fn sidebar_page_sets_the_density(cx: &mut TestAppContext) {
    let root = store("density");
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
    let (view, cx) = open_sheet(cx, Some(host));
    wait_for_save(&view, cx);
    press(cx, "settings-page-sidebar");
    press(cx, "settings-sidebar-Roomy");
    assert_eq!(
        view.read_with(cx, |this, _| this.shell.sidebar.density),
        SidebarDensity::Roomy
    );
    wait_for_save(&view, cx);
    assert_eq!(document(&root)["sidebar_density"], "roomy");
    drop(view);
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("{error}"));
}

#[gpui::test]
fn density_chosen_before_load_survives_a_saved_roomy(cx: &mut TestAppContext) {
    let root = store("before-load");
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(
        root.join("yebisu-settings.json"),
        serde_json::json!({
            "version": 1,
            "language": "en-US",
            "compact": false,
            "terminal_priority": false,
            "board_hidden": false,
            "sidebar_density": "roomy",
            "drag_select": false,
        })
        .to_string(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
    let (view, cx) = open_sheet(cx, Some(host));
    assert!(!view.read_with(cx, |this, _| {
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }));
    press(cx, "settings-page-sidebar");
    press(cx, "settings-sidebar-Dense");
    wait_for_save(&view, cx);
    let stored = document(&root);
    assert_eq!(
        stored.get("sidebar_density"),
        None,
        "Dense is the stored default"
    );
    assert_eq!(
        stored["drag_select"], false,
        "the other loaded option remains"
    );
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.settings_values().sidebar_density,
            SidebarDensity::Dense
        );
        assert_eq!(this.shell.sidebar.density, SidebarDensity::Dense);
    });
    drop(view);
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("{error}"));
}

#[gpui::test]
fn tapping_the_backdrop_closes_the_sheet(cx: &mut TestAppContext) {
    let (view, cx) = open_sheet(cx, None);
    let panel = bounds(cx, "settings-panel");
    let inside = point(panel.left() + px(10.), panel.top() + px(8.));
    cx.simulate_click(inside, Default::default());
    assert!(view.read_with(cx, |this, _| this.settings_modal));
    let outside = point(panel.left() - px(12.), panel.center().y);
    cx.simulate_click(outside, Default::default());
    assert!(!view.read_with(cx, |this, _| this.settings_modal));
}

#[gpui::test]
fn input_shortcut_style_writes_both_controls_to_the_settings_store(cx: &mut TestAppContext) {
    let root = store("shortcut-style");
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("{error}"));
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
    let (view, cx) = open_sheet(cx, Some(host));
    wait_for_save(&view, cx);
    press(cx, "settings-page-input");

    let ipad = bounds(cx, "settings-shortcut-style-ipad");
    let herdr = bounds(cx, "settings-shortcut-style-herdr");
    assert_eq!(ipad.size.height, px(40.));
    assert_eq!(ipad.right(), herdr.left());
    let switch = bounds(cx, "settings-server-keybindings");
    assert_eq!(switch.size, size(px(52.), px(32.)));
    assert_eq!(
        herdr.right() + px(2.),
        switch.right(),
        "inline controls share the right edge"
    );
    assert!(herdr.origin.y < bounds(cx, "settings-option-key-0").origin.y);
    assert!(!view.read_with(cx, |this, _| this.actions.herdr_prefix));
    assert!(view.read_with(cx, |this, _| this.actions.use_server_keybindings));

    press(cx, "settings-shortcut-style-herdr");
    press(cx, "settings-server-keybindings");
    wait_for_save(&view, cx);
    let stored = document(&root);
    assert_eq!(stored["shortcut_style"], "herdr-prefix");
    assert_eq!(stored["server_keybindings"], "off");
    assert!(view.read_with(cx, |this, _| this.actions.herdr_prefix));
    assert!(!view.read_with(cx, |this, _| this.actions.use_server_keybindings));
    drop(view);
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("{error}"));
}
