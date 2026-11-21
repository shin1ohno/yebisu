use super::*;
use crate::shell::{entry_move::LegacyBands, entry_points};

/// Hosts and Settings share the footer. New space belongs in the header.
#[test]
fn sidebar_footer_has_new_space_hosts_settings() {
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        let items: Vec<_> = footer_items(kind).iter().map(|e| e.item).collect();
        assert_eq!(items, [FooterItem::Hosts, FooterItem::Settings], "{kind:?}");
        let state = ChromeState {
            kind,
            connected: true,
            ..ChromeState::default()
        };
        let points = footer_entry_points(&state);
        assert!(points.iter().any(|point| {
            point.action == EntryAction::NewSpace
                && point.place == Place::SidebarHeader
                && point.enabled
                && point.reason.is_none()
        }));
        assert!(!points.iter().any(|point| {
            point.action == EntryAction::NewSpace && point.place == Place::SidebarFooter
        }));
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
    assert_eq!(FooterItem::Settings.label(), "Settings");
    assert_eq!(FOOTER_ROW_HEIGHT, 44.0);
}

#[gpui::test]
fn sidebar_footer_height_matches_the_rendered_footer(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let footer = cx
        .debug_bounds("shell-sidebar-footer")
        .unwrap_or_else(|| panic!("sidebar footer not painted"));
    assert_eq!(footer.size.height, px(crate::layout::SIDEBAR_FOOTER_HEIGHT));
}
