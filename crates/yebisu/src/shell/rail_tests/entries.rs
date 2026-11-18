use crate::shell::{
    ChromeState, EntryAction, HerdrKind, Place,
    rail::{new_space_unavailable, rail_entry_points},
};

fn hidden(kind: HerdrKind, connected: bool) -> ChromeState {
    ChromeState {
        kind,
        connected,
        sidebar_hidden: true,
        ..ChromeState::default()
    }
}

#[test]
fn rail_offers_hosts_settings_new_space_and_the_toggle() {
    let entries = rail_entry_points(&hidden(HerdrKind::Herdr1, true));
    let actions: Vec<_> = entries.iter().map(|e| e.action).collect();
    assert_eq!(
        actions,
        [
            EntryAction::Hosts,
            EntryAction::Settings,
            EntryAction::NewSpace,
            EntryAction::ToggleSidebar
        ]
    );
    assert!(entries.iter().all(|e| e.place == Place::Rail && e.enabled));
}

#[test]
fn rail_entries_exist_only_while_the_rail_is_shown() {
    let shown = ChromeState {
        sidebar_hidden: false,
        ..ChromeState::default()
    };
    assert!(rail_entry_points(&shown).is_empty());
}

#[test]
fn new_space_is_disabled_with_a_reason_where_it_cannot_work() {
    for (kind, connected, reason) in [
        (HerdrKind::Herdr2, true, "Not available on Herdr 2"),
        (HerdrKind::Herdr1, false, "Not connected"),
    ] {
        let entries = rail_entry_points(&hidden(kind, connected));
        let new_space = entries.iter().find(|e| e.action == EntryAction::NewSpace);
        let new_space = new_space.map(|e| (e.enabled, e.reason));
        assert_eq!(new_space, Some((false, Some(reason))));
        assert_eq!(new_space_unavailable(kind, connected), Some(reason));
    }
    assert_eq!(new_space_unavailable(HerdrKind::Herdr1, true), None);
}
