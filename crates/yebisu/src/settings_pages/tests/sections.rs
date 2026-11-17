//! q7, S-1: eight sections in canvas order, no Sidebar, every page placed.
use crate::settings_pages::{
    Page,
    sections::{GROUPS, Section, section_for_page, sections},
};

#[test]
fn settings_sections_follow_canvas_order() {
    assert_eq!(
        sections(),
        [
            Section::Display,
            Section::Theme,
            Section::Input,
            Section::Shortcuts,
            Section::Connection,
            Section::Notifications,
            Section::HostsAndKeys,
            Section::About,
        ]
    );
    assert_eq!(
        sections()
            .into_iter()
            .map(Section::title)
            .collect::<Vec<_>>(),
        [
            "Display",
            "Theme",
            "Input",
            "Shortcuts",
            "Connection",
            "Notifications",
            "Hosts and keys",
            "About"
        ]
    );
    assert_eq!(
        GROUPS.map(|(name, members)| (name, members.len())),
        [("APPEARANCE", 2), ("BEHAVIOUR", 4), ("CONNECT", 2)]
    );
}

#[test]
fn no_sidebar_section() {
    assert_eq!(sections().len(), 8);
    for section in sections() {
        assert!(
            !section.title().contains("Sidebar"),
            "{section:?} is a Sidebar section"
        );
    }
    for (group, _) in GROUPS {
        assert!(!group.eq_ignore_ascii_case("sidebar"));
    }
}

#[test]
fn every_existing_page_has_a_section() {
    // The six pages that shipped before L1 (S-1).
    for (page, section) in [
        (Page::Fonts, Section::Display),
        (Page::Appearance, Section::Theme),
        (Page::Keyboard, Section::Shortcuts),
        (Page::DeviceKey, Section::HostsAndKeys),
        (Page::General, Section::About),
        (Page::Notifications, Section::Notifications),
    ] {
        assert_eq!(section_for_page(page), Some(section), "{page:?}");
    }
    for page in Page::ALL {
        let section = section_for_page(page);
        assert!(section.is_some(), "{page:?} has no section");
        assert_eq!(
            section.map(Section::page),
            Some(page),
            "{page:?} round trip"
        );
    }
    for section in sections() {
        assert_eq!(section_for_page(section.page()), Some(section));
    }
}
