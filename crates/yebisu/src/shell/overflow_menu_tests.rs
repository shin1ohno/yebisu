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
            Paste, PasteImage, SelectText, Copy, Find, Reconnect, Detach, NewTab
        ]
    );
    // Find only when PR 26's `available` is true.
    let without_find = overflow_menu_items(&state(HerdrKind::Herdr2, true, false, false));
    assert_eq!(
        items(&without_find),
        [
            Paste, PasteImage, SelectText, Copy, Reconnect, Detach, NewTab
        ]
    );
    // No command palette and no Close tab on Herdr 2.
    assert!(find(&with_find, CommandPalette).is_none() && find(&with_find, CloseTab).is_none());
    // New tab is disabled with its reason (q4).
    assert_eq!(
        find(&with_find, NewTab).cloned(),
        Some(OverflowEntry {
            item: NewTab,
            enabled: false,
            reason: Some(HERDR2_NEW_TAB)
        })
    );
    assert_eq!(find(&with_find, Detach).map(|e| e.enabled), Some(true));
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
