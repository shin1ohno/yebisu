//! Forward port… (issue 140): enabled for Herdr 1 and 2, connected or not;
//! disabled only by an open sheet; on another host's row it stays enabled
//! while the other items say "Select this host first"; it opens the sheet
//! for the host it was shown for, which must still be saved.
use super::super::*;
use super::support::smoke;
use crate::shell::sidebar::long_press::PressedRow;
use gpui::{TestAppContext, point, px};

const FORWARD: MenuItemId = MenuItemId::Host(HostMenuItem::ForwardPort);

fn forward_entry(entries: &[MenuEntry]) -> MenuEntry {
    entries
        .iter()
        .find(|entry| entry.item == FORWARD)
        .cloned()
        .unwrap_or_else(|| panic!("no Forward port… in {entries:?}"))
}

fn host_menu(state: &ChromeState) -> Vec<MenuEntry> {
    menu_bar(state, true)
        .into_iter()
        .find(|(title, _)| *title == HOST_MENU)
        .map(|(_, entries)| entries)
        .unwrap_or_default()
}

#[test]
fn enabled_in_every_kind_and_link() {
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        for connected in [false, true] {
            let state = ChromeState {
                kind,
                connected,
                host_selected: true,
                ..ChromeState::default()
            };
            let entry = forward_entry(&host_long_press(&state));
            assert!(entry.enabled, "{kind:?} connected={connected}");
            assert_eq!(entry.reason, None);
            assert!(forward_entry(&host_menu(&state)).enabled);
        }
    }
    assert_eq!(FORWARD.label(), "Forward port…");
    assert_eq!(FORWARD.name(), "host.forward_port");
    assert_eq!(MenuItemId::from_name("host.forward_port"), Some(FORWARD));
    assert_eq!(menu_item_label(FORWARD, "studio"), "Forward port… — studio");
}

#[test]
fn sheet_open_disables() {
    let state = ChromeState {
        connected: true,
        host_selected: true,
        sheet_open: true,
        ..ChromeState::default()
    };
    for entry in [
        forward_entry(&host_long_press(&state)),
        forward_entry(&host_menu(&state)),
    ] {
        assert!(!entry.enabled);
        assert_eq!(entry.reason, Some("Close the sheet first"));
    }
}

#[test]
fn menu_bar_needs_a_selected_host() {
    let state = ChromeState::default();
    let entry = forward_entry(&host_menu(&state));
    assert!(!entry.enabled);
    assert_eq!(entry.reason, Some("Select a host first"));
    // A row names its own host, so the long press does not need one.
    assert!(forward_entry(&host_long_press(&state)).enabled);
}

#[test]
fn other_host_row_keeps_forward() {
    let state = ChromeState {
        connected: true,
        host_selected: true,
        ..ChromeState::default()
    };
    let current = MenuScope {
        host: "studio".into(),
        space: None,
    };
    let entries = host_long_press_for(&state, "mini", &current);
    for entry in &entries {
        if entry.item == FORWARD {
            assert!(entry.enabled, "Forward port… runs on the row's host");
        } else {
            assert!(!entry.enabled);
            assert_eq!(entry.reason, Some("Select this host first"), "{entry:?}");
        }
    }
    assert_eq!(entries.len(), 4);
    // An open sheet still wins on another host's row.
    let open = ChromeState {
        sheet_open: true,
        ..state
    };
    assert!(!forward_entry(&host_long_press_for(&open, "mini", &current)).enabled);
}

#[gpui::test]
fn run_on_other_host_opens_sheet(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "studio");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.setup.hosts = vec!["studio".into(), "mini".into()];
            // Pressed on mini's row while studio is selected (owner decision 7).
            this.sidebar_long_press(PressedRow::Host("mini".into()), point(px(10.), px(10.)));
            let menu = this
                .shell
                .sidebar
                .long_press
                .clone()
                .unwrap_or_else(|| panic!("no menu"));
            assert!(forward_entry(&menu.entries).enabled);
            if let Err(error) = this.pick_long_press(FORWARD, window, cx) {
                panic!("Forward port… refused: {error}");
            }
            assert_eq!(this.shell.forward_sheet.host.as_deref(), Some("mini"));
            assert!(this.forward_sheet_open());
            // The open sheet now disables the item everywhere.
            assert!(this.menu_chrome_state().sheet_open);
            assert!(
                this.run_menu_item(FORWARD, &menu.shown, window, cx)
                    .is_err()
            );
        })
    });
}

#[gpui::test]
fn unsaved_host_refused(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "studio");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.setup.hosts = vec!["studio".into()];
            let shown = MenuScope {
                host: "gone".into(),
                space: None,
            };
            let refused = this.run_menu_item(FORWARD, &shown, window, cx);
            assert!(refused.is_err(), "a host no longer saved is refused");
            assert!(!this.forward_sheet_open());
            let empty = MenuScope::default();
            assert!(this.run_menu_item(FORWARD, &empty, window, cx).is_err());
            assert!(!this.forward_sheet_open());
            let saved = MenuScope {
                host: "studio".into(),
                space: None,
            };
            assert!(this.run_menu_item(FORWARD, &saved, window, cx).is_ok());
            assert!(this.forward_sheet_open());
        })
    });
}
