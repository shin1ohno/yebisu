use super::*;
use crate::shell::{
    EntryAction,
    entry_move::LegacyBands,
    entry_points::{chrome_entry_points, required_entry_actions},
    frame::COLUMN_ORDER,
    overflow_menu::{HERDR2_NEW_TAB, OverflowItem, overflow_menu_items},
    status_bar::{status_bar_model, status_line},
};

fn item(state: &ChromeState, item: OverflowItem) -> crate::shell::overflow_menu::OverflowEntry {
    overflow_menu_items(state)
        .into_iter()
        .find(|e| e.item == item)
        .unwrap_or_else(|| panic!("{item:?} missing from the ... menu"))
}

#[test]
fn herdr2_uses_shared_chrome() {
    for connected in [true, false] {
        let two = herdr2(connected);
        let one = ChromeState {
            kind: HerdrKind::Herdr1,
            ..two
        };
        // The status bar is the only place that names the daemon.
        let line_two = status_line(&status_bar_model(&two, "pro-dev"));
        let line_one = status_line(&status_bar_model(&one, "pro-dev"));
        assert_eq!(line_two, line_one.replace("Herdr 1", "Herdr 2"));
        assert!(line_two.contains("Herdr 2"), "{line_two}");
        // The same entry points reach every required action.
        let offered = chrome_entry_points(&two, LegacyBands::current())
            .into_iter()
            .map(|p| p.action)
            .collect::<Vec<_>>();
        for action in required_entry_actions(&two) {
            assert!(offered.contains(&action), "{action:?} missing on Herdr 2");
        }
    }
    // One column order for both kinds: no Herdr 2 variant of the frame.
    assert_eq!(COLUMN_ORDER.len(), 6);
    // The sidebar column has Herdr 2 content to draw: host row and session.
    let state = SidebarState::default();
    let host = connected_host("pro-dev", &[raw("t1", Some("working"))], &state);
    assert_eq!(host.kind, HerdrKind::Herdr2);
    assert_eq!(host.spaces.len(), 1);
    assert_eq!(host.agents.len(), 1);
}

#[test]
fn herdr2_new_tab_disabled_with_reason() {
    for connected in [true, false] {
        let entry = item(&herdr2(connected), OverflowItem::NewTab);
        assert!(!entry.enabled);
        assert_eq!(entry.reason, Some(HERDR2_NEW_TAB));
    }
    // The entry point stays listed (disabled, with its reason), not removed.
    let points = chrome_entry_points(&herdr2(true), LegacyBands::current());
    assert!(points.iter().any(|p| p.action == EntryAction::NewTab));
}

#[test]
fn herdr2_paste_image_enabled() {
    let entry = item(&herdr2(true), OverflowItem::PasteImage);
    assert!(entry.enabled, "q2: Herdr 2 pastes images since PR 37");
    assert_eq!(entry.reason, None);
    let points = chrome_entry_points(&herdr2(true), LegacyBands::current());
    assert!(
        points
            .iter()
            .any(|p| p.action == EntryAction::PasteImage && p.enabled)
    );
}
