use super::*;
use gpui::{AppContext as _, TestAppContext, px, size};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn dimensions_motion_and_strings_use_source_constants() {
    assert_eq!(
        sheet::sheet_size(size(px(1400.), px(1000.))),
        size(px(1000.), px(760.))
    );
    assert_eq!(
        sheet::sheet_size(size(px(20.), px(20.))),
        size(px(0.), px(0.))
    );
    assert!(strings::valid());
    assert_eq!(strings::text("settings-title"), "Settings");
    assert_eq!(appearance::Appearance::for_density(true).size, 12.);
    assert_eq!(appearance::Appearance::for_density(false).line_height, 20.);
    assert!(appearance::FONT_SIZE_RANGE.contains(&14.));
    let now = Instant::now();
    let mut toggle = motion::Toggle::default();
    toggle.set(true, now, true);
    toggle.set(false, now, false);
    assert!(toggle.moving(now));
    assert_eq!(toggle.shown(now + motion::LEAVE), 0.);
    assert!(!draft::available("config-shared"));
    assert!(draft::available("fonts-density"));
}

#[gpui::test]
fn copied_controls_and_sheet_render_and_flush_real_owner(cx: &mut TestAppContext) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/settings-spike-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let mut host =
        crate::settings::SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !host.ready() {
        host.poll();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let owner = cx.new(crate::app::Smoke::new);
    owner.update(cx, |owner, _| owner.settings = Some(host));
    let (sheet, cx) =
        cx.add_window_view(|window, cx| sheet::SettingsSheet::install(owner.clone(), window, cx));
    cx.simulate_resize(size(px(1100.), px(900.)));
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("settings-spike-panel")
        .unwrap_or_else(|| panic!("sheet did not render"));
    assert_eq!(bounds.size, size(px(1000.), px(760.)));
    assert!(cx.debug_bounds("font-specimen").is_some());
    let plus = cx
        .debug_bounds("font-size-spike-plus")
        .unwrap_or_else(|| panic!("number stepper absent"));
    cx.simulate_click(plus.center(), gpui::Modifiers::default());
    assert_eq!(
        sheet.read_with(cx, |sheet, cx| sheet.editor.read(cx).font_size),
        15.
    );
    let switch = cx
        .debug_bounds("keyboard-priority-toggle")
        .unwrap_or_else(|| panic!("copied switch absent"));
    cx.simulate_click(switch.center(), gpui::Modifiers::default());
    assert!(owner.read_with(cx, |owner, _| owner.actions.terminal_priority));
    cx.update(|window, cx| sheet.update(cx, |sheet, cx| sheet.dismiss(window, cx)));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        owner.update(cx, |owner, cx| owner.poll_settings(cx));
        cx.update(|window, cx| sheet.update(cx, |sheet, cx| sheet.dismiss(window, cx)));
        if !sheet.read_with(cx, |sheet, _| sheet.open) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let saved: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(saved["terminal_priority"], true);
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
