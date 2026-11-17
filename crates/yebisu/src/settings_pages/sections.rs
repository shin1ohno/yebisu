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

/// The sections in canvas order. Wave 0: none yet (the sheet keeps its pages).
pub(crate) fn sections() -> Vec<Section> {
    Vec::new()
}

/// Where an existing page moves (S-1). Wave 0: nowhere yet.
pub(crate) fn section_for_page(_page: Page) -> Option<Section> {
    None
}
