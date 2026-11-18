use super::*;
use crate::{
    layout::SIDEBAR_FOOTER_HEIGHT,
    shell::{entry_move::LegacyBands, entry_points},
};

/// Plan v2 7.6: New space, Hosts and Settings under the sidebar, each 44.
#[test]
fn sidebar_footer_has_new_space_hosts_settings() {
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        let items: Vec<_> = footer_items(kind).iter().map(|e| e.item).collect();
        assert_eq!(
            items,
            [
                FooterItem::NewSpace,
                FooterItem::Hosts,
                FooterItem::Settings
            ],
            "{kind:?}"
        );
        let state = ChromeState {
            kind,
            connected: true,
            ..ChromeState::default()
        };
        let points = footer_entry_points(&state);
        for action in [
            EntryAction::NewSpace,
            EntryAction::Hosts,
            EntryAction::Settings,
        ] {
            assert!(
                points
                    .iter()
                    .any(|p| p.action == action && p.place == Place::SidebarFooter),
                "{kind:?}: {action:?} not in the footer"
            );
        }
        // The frame model sees them, and New space is now required (A4).
        let all = entry_points::chrome_entry_points(&state, LegacyBands::current());
        assert!(
            entry_points::required_entry_actions(&state).contains(&EntryAction::NewSpace),
            "{kind:?}"
        );
        for action in [EntryAction::Hosts, EntryAction::Settings] {
            assert!(
                all.iter()
                    .any(|p| p.action == action && p.place == Place::SidebarFooter && p.enabled),
                "{kind:?}: {action:?}"
            );
        }
    }
    // A hidden sidebar draws no footer, so it offers no place.
    let hidden = ChromeState {
        sidebar_hidden: true,
        ..ChromeState::default()
    };
    assert!(footer_entry_points(&hidden).is_empty());
    assert_eq!(FooterItem::NewSpace.label(), "New space");
    assert_eq!(FooterItem::Hosts.label(), "Hosts");
    // SmokeQA taps the "Settings" text (SmokeQA+Settings.swift:15).
    assert_eq!(FooterItem::Settings.label(), "Settings");
    // Two 44 rows, padding 4 + 4 and the rule fill L0's footer height.
    assert_eq!(FOOTER_ROW_HEIGHT, 44.0);
    assert_eq!(
        1.0 + 2.0 * FOOTER_PADDING + 2.0 * FOOTER_ROW_HEIGHT,
        SIDEBAR_FOOTER_HEIGHT
    );
}
