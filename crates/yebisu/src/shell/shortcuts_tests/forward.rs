//! Issue 140, board 45: the APP group's Forward port… under Settings, with
//! no key; disabled "Select a host first" without a selected host.
use super::super::*;
use gpui::{Modifiers, TestAppContext, px, size};

#[test]
fn disabled_without_host() {
    let none = ChromeState::default();
    let row = forward_port_row(&none);
    assert_eq!(row.label, "Forward port…");
    assert_eq!(row.keys, "", "no key is shown");
    assert!(!row.enabled);
    assert_eq!(forward_port_reason(&none), Some("Select a host first"));
    let selected = ChromeState {
        host_selected: true,
        ..none
    };
    assert!(forward_port_row(&selected).enabled);
    assert_eq!(forward_port_reason(&selected), None);
    let sheet = ChromeState {
        sheet_open: true,
        ..selected
    };
    assert!(!forward_port_row(&sheet).enabled);
    assert_eq!(forward_port_reason(&sheet), Some("Close the sheet first"));
    // Its entry point says why when disabled (q4).
    let point = shortcuts_entry_points(&none)
        .into_iter()
        .find(|p| p.action == EntryAction::ForwardPort && p.place == Place::ShortcutsPopover)
        .unwrap_or_else(|| panic!("no popover entry point"));
    assert!(!point.enabled);
    assert_eq!(point.reason, Some("Select a host first"));
}

#[gpui::test]
fn app_group_row_after_settings(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(1000.)));
    view.update(cx, |this, cx| {
        this.setup.hosts = vec!["me@pro-dev".into()];
        this.terminal.reconnect.target = "me@pro-dev".into();
        assert!(this.open_shortcuts(cx));
    });
    cx.run_until_parked();
    let settings = cx
        .debug_bounds("shell-shortcut-Settings")
        .unwrap_or_else(|| panic!("no Settings row"));
    let forward = cx
        .debug_bounds("shell-shortcut-Forward port…")
        .unwrap_or_else(|| panic!("no Forward port… row"));
    assert_eq!(forward.left(), settings.left(), "same APP column");
    assert!(
        forward.top() >= settings.bottom(),
        "{forward:?} under {settings:?}"
    );
    // A tap opens the sheet for the selected host.
    cx.simulate_click(forward.center(), Modifiers::none());
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(!this.shell.shortcuts.open);
        assert_eq!(this.shell.forward_sheet.host.as_deref(), Some("me@pro-dev"));
    });
}
