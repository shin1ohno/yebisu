//! Q8: long press of a host or space row lists the menu bar's items.
use super::super::*;
use super::support::states;

fn menu(state: &ChromeState, title: &str) -> Vec<MenuEntry> {
    menu_bar(state, true)
        .into_iter()
        .find(|(t, _)| *t == title)
        .map(|(_, entries)| entries)
        .unwrap_or_default()
}

#[test]
fn long_press_items_also_in_menu_bar() {
    for state in states() {
        for entry in host_long_press(&state) {
            assert!(menu(&state, HOST_MENU).contains(&entry));
        }
        for entry in space_long_press(&state) {
            assert!(menu(&state, SPACE_MENU).contains(&entry));
        }
    }
    // The menu bar has exactly the Host and Space menus, in that order.
    let titles: Vec<_> = menu_bar(&ChromeState::default(), true)
        .into_iter()
        .map(|(title, _)| title)
        .collect();
    assert_eq!(titles, [HOST_MENU, SPACE_MENU]);
}

#[test]
fn menu_items_match_long_press() {
    for state in states() {
        assert_eq!(menu(&state, HOST_MENU), host_long_press(&state));
        assert_eq!(menu(&state, SPACE_MENU), space_long_press(&state));
    }
    // Q8's items, by label.
    let labels = |entries: Vec<MenuEntry>| -> Vec<&str> {
        entries.into_iter().map(|e| e.item.label()).collect()
    };
    let state = ChromeState::default();
    assert_eq!(
        labels(host_long_press(&state)),
        ["Reconnect", "Detach", "Edit"]
    );
    assert_eq!(
        labels(space_long_press(&state)),
        ["New tab", "Detach", "Rename Space"]
    );
    // Every item name round-trips to its item (the menu bar dispatch).
    for (_, entries) in menu_bar(&state, true) {
        for entry in entries {
            assert_eq!(MenuItemId::from_name(entry.item.name()), Some(entry.item));
        }
    }
    // Disabled items carry their reason (q4).
    let herdr1 = host_long_press(&ChromeState {
        connected: true,
        ..state
    });
    assert!(herdr1.iter().all(|e| e.enabled || e.reason.is_some()));
}

#[test]
fn long_names_end_in_ellipsis() {
    let host = "h".repeat(40);
    let label = menu_item_label(MenuItemId::Host(HostMenuItem::Reconnect), &host);
    assert!(label.ends_with('…'));
    assert_eq!(menu_name(&host).chars().count(), MENU_NAME_MAX);
    let space = "日本語のとても長いスペースの名前がここに入りますのでご注意ください";
    assert!(menu_name(space).ends_with('…'));
    assert_eq!(menu_name("studio"), "studio");
}

#[test]
fn long_press_carries_the_pressed_row() {
    let state = ChromeState {
        connected: true,
        ..ChromeState::default()
    };
    let current = MenuScope {
        host: "alpha".into(),
        space: Some("w1".into()),
    };
    // The selected host's row lists the host items as the menu bar does.
    assert_eq!(
        host_long_press_for(&state, "alpha", &current),
        host_long_press(&state)
    );
    // Another host's row would run them on "alpha": disabled, with why.
    for e in host_long_press_for(&state, "beta", &current) {
        assert!(
            !e.enabled && e.reason == Some("Select this host first"),
            "{e:?}"
        );
    }
    let row = |host: &str, space: &str| MenuScope {
        host: host.into(),
        space: Some(space.into()),
    };
    assert_eq!(
        space_long_press_for(&state, &row("alpha", "w1"), &current),
        space_long_press(&state)
    );
    // Issue #143 B-3: Rename names any space of the host, so it alone
    // stays usable on another space's row.
    for e in space_long_press_for(&state, &row("alpha", "w2"), &current) {
        if e.item == MenuItemId::Space(SpaceMenuItem::Rename) {
            assert!(e.enabled, "{e:?}");
            continue;
        }
        assert!(
            !e.enabled && e.reason == Some("Open this space first"),
            "{e:?}"
        );
    }
    for e in space_long_press_for(&state, &row("beta", "w1"), &current) {
        assert!(
            !e.enabled && e.reason == Some("Select this host first"),
            "{e:?}"
        );
    }
}

#[test]
fn items_are_disabled_while_a_sheet_is_open() {
    for state in states().into_iter().filter(|s| s.sheet_open) {
        let all = menu_bar(&state, true)
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .chain(host_long_press(&state))
            .chain(space_long_press(&state));
        for e in all {
            assert!(
                !e.enabled && e.reason == Some("Close the sheet first"),
                "{e:?}"
            );
        }
    }
}
