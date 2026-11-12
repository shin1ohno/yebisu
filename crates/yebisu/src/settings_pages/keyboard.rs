//! Read-only supported shortcut catalog and the persisted terminal priority.

use crate::{actions, settings::Values, terminal_view::TerminalView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutRow {
    pub label: String,
    pub shortcuts: Vec<String>,
    pub available: bool,
}

pub(crate) fn shortcut_rows(terminal: &TerminalView) -> Vec<ShortcutRow> {
    actions::catalog(terminal)
        .into_iter()
        .map(|action| ShortcutRow {
            label: action.label,
            shortcuts: action.shortcuts.iter().map(|shortcut| (*shortcut).to_owned()).collect(),
            available: action.available,
        })
        .collect()
}

pub(crate) const fn terminal_priority(values: &Values) -> bool {
    values.terminal_priority
}

/// Edits the setting already read by `actions::shortcut`.
pub(crate) fn set_terminal_priority(values: &mut Values, enabled: bool) {
    values.terminal_priority = enabled;
}
