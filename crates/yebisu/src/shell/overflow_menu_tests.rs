use super::*;
use OverflowItem::*;

fn state(kind: HerdrKind, connected: bool, selecting: bool, find: bool) -> ChromeState {
    ChromeState {
        kind,
        connected,
        selecting,
        find_available: find,
        ..ChromeState::default()
    }
}

fn items(entries: &[OverflowEntry]) -> Vec<OverflowItem> {
    entries.iter().map(|e| e.item).collect()
}

fn find(entries: &[OverflowEntry], item: OverflowItem) -> Option<&OverflowEntry> {
    entries.iter().find(|e| e.item == item)
}

#[test]
fn overflow_menu_items_for_herdr1() {
    let selecting = overflow_menu_items(&state(HerdrKind::Herdr1, true, true, true));
    assert_eq!(
        items(&selecting),
        [
            Paste,
            PasteImage,
            SelectText,
            Copy,
            Find,
            CommandPalette,
            Reconnect,
            CloseTab
        ]
    );
    // Copy only while there is a selection.
    let idle = overflow_menu_items(&state(HerdrKind::Herdr1, true, false, true));
    assert_eq!(
        items(&idle),
        [
            Paste,
            PasteImage,
            SelectText,
            Find,
            CommandPalette,
            Reconnect,
            CloseTab
        ]
    );
    // Herdr 1 never offers Detach or a disabled New tab here.
    assert!(find(&idle, Detach).is_none() && find(&idle, NewTab).is_none());
    // Reconnect is enabled only while disconnected; Find follows `available`.
    let reconnect = find(&idle, Reconnect).map(|e| e.enabled);
    assert_eq!(reconnect, Some(false));
    let offline = overflow_menu_items(&state(HerdrKind::Herdr1, false, false, false));
    assert_eq!(find(&offline, Reconnect).map(|e| e.enabled), Some(true));
    let find_entry = find(&offline, Find).cloned();
    assert_eq!(
        find_entry,
        Some(OverflowEntry {
            item: Find,
            enabled: false,
            reason: Some(FIND_UNAVAILABLE)
        })
    );
    // Every disabled entry says why.
    for entry in offline.iter().chain(&idle) {
        assert_eq!(entry.enabled, entry.reason.is_none(), "{entry:?}");
    }
}

#[test]
fn overflow_menu_items_for_herdr2() {
    let with_find = overflow_menu_items(&state(HerdrKind::Herdr2, true, false, true));
    assert_eq!(
        items(&with_find),
        [
            NewTab,
            Split,
            Paste,
            PasteImage,
            Reconnect,
            Detach,
            SelectText,
            Copy,
            Find,
            Pane1,
            Pane2,
            SplitDirection
        ]
    );
    assert_eq!(
        HERDR2_TAB_COMMANDS.map(|(_, _, label, keys)| (label, keys)),
        [
            ("New tab", "⌘T"),
            ("Split right", "⌘D"),
            ("Split down", "⇧⌘D")
        ]
    );
    let without_find = overflow_menu_items(&state(HerdrKind::Herdr2, true, false, false));
    assert!(find(&without_find, Find).is_none());
    for item in [NewTab, Split, Paste, PasteImage, Detach] {
        assert!(find(&with_find, item).is_some_and(|entry| entry.enabled));
    }
    let offline = overflow_menu_items(&state(HerdrKind::Herdr2, false, false, false));
    assert!(find(&offline, Reconnect).is_some_and(|entry| entry.enabled));
    assert!(find(&offline, NewTab).is_some_and(|entry| entry.reason == Some(NOT_CONNECTED)));
}

#[test]
fn herdr2_overflow_paste_image_enabled() {
    let entries = overflow_menu_items(&state(HerdrKind::Herdr2, true, false, false));
    assert_eq!(
        find(&entries, PasteImage).cloned(),
        Some(OverflowEntry {
            item: PasteImage,
            enabled: true,
            reason: None
        })
    );
}

/// Review 9c55 S3: with the band controls in the Herdr 2 menu, the band
/// retires; without any one of them, or with one disabled, it stays.
#[test]
fn herdr2_menu_retires_the_band_only_with_every_control() {
    use crate::shell::herdr2_frame::panes::{band_retired, band_retired_in};
    let connected = state(HerdrKind::Herdr2, true, false, false);
    let menu = overflow_menu_items(&connected);
    assert!(band_retired_in(&menu));
    assert!(band_retired(&connected));
    for removed in [Split, Pane1, Pane2, SplitDirection, Detach] {
        let without = menu
            .iter()
            .filter(|e| e.item != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(!band_retired_in(&without), "{removed:?} removed");
        let mut disabled = menu.clone();
        for e in &mut disabled {
            if e.item == removed {
                e.enabled = false;
            }
        }
        assert!(!band_retired_in(&disabled), "{removed:?} disabled");
    }
    // Offline the items are disabled, but homes are judged connected: the
    // band does not come back while the link is down.
    assert!(band_retired(&state(HerdrKind::Herdr2, false, false, false)));
    // Herdr 1's menu never holds them.
    assert!(!band_retired_in(&overflow_menu_items(&state(
        HerdrKind::Herdr1,
        true,
        false,
        false
    ))));
}

#[test]
fn overflow_menu_matches_the_canvas_geometry() {
    use super::super::style::{metric, radius, shadow};
    assert_eq!(MENU_RIGHT, 8.);
    assert_eq!(MENU_TOP, 40.);
    assert_eq!(metric::MENU_WIDTH, 260.);
    assert_eq!(metric::MENU_ITEM, 48.);
    assert_eq!(metric::MENU_PADDING, 6.);
    assert_eq!(radius::POPOVER, 14.);
    assert_eq!((shadow::POPOVER.y, shadow::POPOVER.blur), (12., 36.));
}

#[test]
fn copy_as_one_line_is_not_in_the_overflow_menu() {
    use super::super::edit_bar::edit_bar_items;
    let label = "Copy as One Line";
    assert!(
        edit_bar_items(true)
            .iter()
            .any(|item| item.label() == label)
    );
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        let entries = overflow_menu_items(&state(kind, true, true, true));
        assert!(find(&entries, Copy).is_some());
        assert!(entries.iter().all(|entry| entry.item.label() != label));
    }
}

#[test]
fn compact_adds_new_tab_for_herdr1_and_preserves_the_normal_menu() {
    for online in [false, true] {
        let herdr1 = state(HerdrKind::Herdr1, online, false, false);
        let normal = menu_entries_for_frame(&herdr1, false);
        assert_eq!(normal, overflow_menu_items(&herdr1));
        assert!(!normal.iter().any(|row| row.item == NewTab));
        let compact = menu_entries_for_frame(&herdr1, true);
        assert_eq!(compact[0].item, NewTab);
        assert_eq!(compact[0].enabled, online);
        assert_eq!(compact[1..], normal);

        let herdr2 = state(HerdrKind::Herdr2, online, false, false);
        assert_eq!(
            menu_entries_for_frame(&herdr2, true),
            menu_entries_for_frame(&herdr2, false),
        );
    }
}
