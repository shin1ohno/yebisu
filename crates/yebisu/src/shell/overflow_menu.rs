//! L2a: the tab row's ... menu. Owner: L2a.
use super::ChromeState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverflowItem {
    Paste,
    PasteImage,
    SelectText,
    Copy,
    Find,
    CommandPalette,
    Reconnect,
    CloseTab,
    Detach,
    NewTab,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OverflowEntry {
    pub(crate) item: OverflowItem,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

/// The ... menu for Herdr 1 or Herdr 2 (plan v2 7.5).
pub(crate) fn overflow_menu_items(_state: &ChromeState) -> Vec<OverflowEntry> {
    Vec::new()
}
