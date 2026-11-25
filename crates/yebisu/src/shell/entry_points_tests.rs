use super::*;

/// The five states of plan v2 5.1 rule 2.
fn states() -> [(&'static str, ChromeState); 5] {
    let base = ChromeState {
        connected: true,
        find_available: true,
        ..ChromeState::default()
    };
    [
        ("Herdr 1 connected", base),
        (
            "Herdr 1 disconnected",
            ChromeState {
                connected: false,
                find_available: false,
                ..base
            },
        ),
        (
            "Herdr 2",
            ChromeState {
                kind: HerdrKind::Herdr2,
                find_available: false,
                ..base
            },
        ),
        (
            "selecting",
            ChromeState {
                selecting: true,
                ..base
            },
        ),
        (
            "sheet open",
            ChromeState {
                sheet_open: true,
                ..base
            },
        ),
    ]
}

#[test]
fn chrome_entry_points_cover_required_set() {
    let legacy = LegacyBands::current();
    for (name, state) in states() {
        let points = chrome_entry_points(&state, legacy);
        for action in required_entry_actions(&state) {
            let places: Vec<_> = points.iter().filter(|p| p.action == action).collect();
            assert!(!places.is_empty(), "{name}: {action:?} has no place");
            // A disabled place must say why (q4).
            for p in &places {
                assert!(p.enabled || p.reason.is_some(), "{name}: {p:?}");
            }
        }
        // The places L2a adds, independent of the legacy bands.
        let at = |action: EntryAction, place: Place| {
            points
                .iter()
                .any(|p| p.action == action && p.place == place)
        };
        assert!(at(EntryAction::Paste, Place::OverflowMenu), "{name}");
        assert!(at(EntryAction::NewTab, Place::TabRow), "{name}");
        assert!(at(EntryAction::Split, Place::TabRow), "{name}");
        assert!(at(EntryAction::Shortcuts, Place::StatusBar), "{name}");
        assert_eq!(
            at(EntryAction::Reconnect, Place::StatusBar),
            !state.connected,
            "{name}"
        );
    }
    // The required set itself (A4: New space waits for L2b's footer).
    let all = required_entry_actions(&states()[1].1);
    for action in [
        EntryAction::Hosts,
        EntryAction::Settings,
        EntryAction::Paste,
        EntryAction::PasteImage,
        EntryAction::Select,
        EntryAction::Copy,
        EntryAction::Reconnect,
        EntryAction::NewTab,
        EntryAction::Split,
        EntryAction::Shortcuts,
        EntryAction::ToggleSidebar,
        EntryAction::ForwardPort,
    ] {
        assert!(all.contains(&action), "{action:?}");
    }
    // Issue 140: Forward port… is in the menu bar, a host row's long press
    // and the Shortcuts list in every state.
    for (name, state) in states() {
        let points = chrome_entry_points(&state, legacy);
        for place in [Place::MenuBar, Place::LongPress, Place::ShortcutsPopover] {
            assert!(
                points
                    .iter()
                    .any(|p| p.action == EntryAction::ForwardPort && p.place == place),
                "{name}: Forward port… missing from {place:?}"
            );
        }
    }
    assert!(!required_entry_actions(&states()[0].1).contains(&EntryAction::Reconnect));
    assert!(required_entry_actions(&states()[0].1).contains(&EntryAction::Find));
    assert_eq!(
        required_entry_actions(&states()[0].1).contains(&EntryAction::NewSpace),
        !footer::footer_items(HerdrKind::Herdr1).is_empty()
    );
}
