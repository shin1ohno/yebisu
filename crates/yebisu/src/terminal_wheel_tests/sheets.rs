//! T6: a sheet over the terminal scrolls its own list under the finger and
//! the terminal gets nothing. The ••• menu's list is checked in
//! `shell/frame_overlay_tests/wave1_follow_up.rs`. The Shortcuts popover's
//! rows carry no debug selector (`shell/shortcuts/view.rs`, `shortcut_row`),
//! so its list is checked through its scroll indicator's handle.
use super::*;
use crate::settings::SettingsHost;
use std::path::PathBuf;

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not painted"))
}

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    let at = bounds(cx, selector).center();
    cx.simulate_click(at, Modifiers::default());
    cx.run_until_parked();
}

/// A finger's pan of `dy` points at `at`, as GPUI delivers it.
fn pan(view: &Entity<Smoke>, cx: &mut VisualTestContext, at: Point<Pixels>, dy: f32) {
    scroll(view, cx, at, pixels(0., dy), TouchPhase::Moved);
    cx.run_until_parked();
}

fn over_the_grid(view: &Entity<Smoke>, cx: &mut VisualTestContext, at: Point<Pixels>) {
    let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
    assert!(
        grid.contains(&at),
        "{at:?} is not over the terminal {grid:?}"
    );
}

/// T6, Settings: the Input page scrolls under the finger over the terminal.
#[gpui::test]
fn the_settings_sheet_scrolls_its_page_over_the_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/issue137-settings-{}",
        std::process::id()
    ));
    view.update(cx, |this, _| {
        this.settings = Some(SettingsHost::open(root).unwrap_or_else(|e| panic!("{e}")));
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until, "settings never loaded");
        std::thread::sleep(Duration::from_millis(2));
    }
    view.update(cx, |this, cx| {
        this.settings_modal = true;
        cx.notify();
    });
    cx.run_until_parked();
    press(cx, "settings-page-input");
    let anchor = "settings-japanese-candidates-0";
    let before = bounds(cx, anchor);
    over_the_grid(&view, cx, before.center());
    pan(&view, cx, before.center(), -230.);
    let after = bounds(cx, anchor);
    assert!(after.top() < before.top(), "{before:?} -> {after:?}");
    assert_eq!(wheels(&view, cx, &wire), []);
}

/// T6, Hosts: the host list, opened from the footer over the terminal,
/// scrolls under the finger.
#[gpui::test]
fn the_hosts_sheet_scrolls_its_list_over_the_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    view.update(cx, |this, cx| {
        this.setup.hosts = (0..30)
            .map(|index| format!("qa{index}@issue-137-{index}.invalid"))
            .collect();
        this.open_hosts(cx);
    });
    cx.run_until_parked();
    let panel = bounds(cx, "hosts-panel");
    let before = bounds(cx, "host-open-0");
    assert!(
        bounds(cx, "host-open-29").bottom() > panel.bottom(),
        "30 hosts fit the panel {panel:?}; nothing to scroll"
    );
    let at = point(before.center().x, panel.center().y);
    over_the_grid(&view, cx, at);
    pan(&view, cx, at, -240.);
    let after = bounds(cx, "host-open-0");
    assert!(after.top() < before.top(), "{before:?} -> {after:?}");
    assert_eq!(wheels(&view, cx, &wire), []);
}

/// T6, Shortcuts: the popover's rows, open over the terminal, scroll under
/// the finger. Its rows have no debug selector, so the rows' scroll handle
/// (the one its indicator reads) tells.
#[gpui::test]
fn the_shortcuts_popover_scrolls_its_rows_over_the_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    install(&view, cx, single(|_| {}));
    // Narrower than the two-column popover (`popover_columns`) and short, so
    // its single column of rows overflows.
    cx.simulate_resize(size(px(780.), px(560.)));
    cx.run_until_parked();
    view.update(cx, |this, cx| {
        this.shell.shortcuts.open = true;
        cx.notify();
    });
    cx.run_until_parked();
    let rows = view.read_with(cx, |this, _| {
        this.shell.indicators.shortcuts.handle().clone()
    });
    assert!(
        rows.max_offset().y > px(0.),
        "the rows fit {:?}; nothing to scroll",
        rows.bounds()
    );
    let at = rows.bounds().center();
    over_the_grid(&view, cx, at);
    let before = rows.offset();
    pan(&view, cx, at, -120.);
    let after = rows.offset();
    assert!(after.y < before.y, "{before:?} -> {after:?}");
    assert_eq!(wheels(&view, cx, &wire), []);
}
