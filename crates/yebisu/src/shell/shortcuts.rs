//! L6: the Shortcuts popover. Owner: L6.
use super::{ChromeState, EntryPoint};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

#[derive(Debug, Default)]
pub(crate) struct ShortcutsState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutRow {
    pub(crate) label: String,
    pub(crate) keys: String,
    pub(crate) enabled: bool,
}

/// Rows from the action catalog (`actions::catalog`).
pub(crate) fn shortcut_rows(_state: &ChromeState) -> Vec<ShortcutRow> {
    Vec::new()
}

/// 3 columns at 760 pt and wider, 1 below.
pub(crate) fn popover_columns(_width: f32) -> u8 {
    3
}

pub(crate) fn shortcuts_entry_points(_state: &ChromeState) -> Vec<EntryPoint> {
    Vec::new()
}

impl Smoke {
    /// Status bar Shortcuts. `false` means not handled: the caller falls
    /// back to the command palette (plan v2 5.2, L2a column).
    pub(crate) fn open_shortcuts(&mut self, _cx: &mut Context<Self>) -> bool {
        false
    }

    pub(crate) fn render_shortcuts_popover(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
}
