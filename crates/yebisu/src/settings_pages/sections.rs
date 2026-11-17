//! L1: the eight settings sections (q7). Owner: L1.
use super::Page;

/// Canvas order; there is no Sidebar section (q7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Section {
    Display,
    Theme,
    Input,
    Shortcuts,
    Connection,
    Notifications,
    HostsAndKeys,
    About,
}

/// The canvas groups (`Settings.dc.html`) and their sections, in order.
/// Notifications follows Connection (owner answer 1 of plan v2 section 10).
pub(crate) const GROUPS: [(&str, &[Section]); 3] = [
    ("APPEARANCE", &[Section::Display, Section::Theme]),
    (
        "BEHAVIOUR",
        &[
            Section::Input,
            Section::Shortcuts,
            Section::Connection,
            Section::Notifications,
        ],
    ),
    ("CONNECT", &[Section::HostsAndKeys, Section::About]),
];

/// The sections in canvas order (the view walks `GROUPS`; tests use this).
#[cfg(test)]
pub(crate) fn sections() -> Vec<Section> {
    GROUPS
        .iter()
        .flat_map(|(_, sections)| sections.iter().copied())
        .collect()
}

/// Where an existing page moves (S-1).
pub(crate) fn section_for_page(page: Page) -> Option<Section> {
    Some(match page {
        Page::Fonts => Section::Display,
        Page::Appearance => Section::Theme,
        // The shortcut list; its priority toggle is the Input page's
        // Shortcut priority row (plan v2 7.4).
        Page::Keyboard => Section::Shortcuts,
        Page::DeviceKey => Section::HostsAndKeys,
        Page::General => Section::About,
        Page::Notifications => Section::Notifications,
        Page::Input => Section::Input,
        Page::Connection => Section::Connection,
    })
}

impl Section {
    /// The label of the section's button (canvas text).
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Theme => "Theme",
            Self::Input => "Input",
            Self::Shortcuts => "Shortcuts",
            Self::Connection => "Connection",
            Self::Notifications => "Notifications",
            Self::HostsAndKeys => "Hosts and keys",
            Self::About => "About",
        }
    }

    /// The page the sheet shows for this section.
    pub(crate) const fn page(self) -> Page {
        match self {
            Self::Display => Page::Fonts,
            Self::Theme => Page::Appearance,
            Self::Input => Page::Input,
            Self::Shortcuts => Page::Keyboard,
            Self::Connection => Page::Connection,
            Self::Notifications => Page::Notifications,
            Self::HostsAndKeys => Page::DeviceKey,
            Self::About => Page::General,
        }
    }
}
