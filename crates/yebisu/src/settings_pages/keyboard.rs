//! Read-only supported shortcut catalog and the persisted terminal priority.

use crate::{
    actions::{self, Actions},
    settings::Values,
    terminal_view::TerminalView,
};
use sheprd_commands::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutRow {
    pub label: String,
    pub shortcuts: Vec<String>,
    pub available: bool,
}

pub(crate) fn shortcut_rows(terminal: &TerminalView, state: &Actions) -> Vec<ShortcutRow> {
    let labels = |command| {
        state
            .host_keys
            .labels(terminal, state.use_server_keybindings, command)
    };
    let mut rows = actions::catalog(terminal)
        .into_iter()
        .map(|action| ShortcutRow {
            label: action.label,
            shortcuts: if state.herdr_prefix {
                labels(action.info.command)
            } else {
                action
                    .shortcuts
                    .iter()
                    .map(|shortcut| (*shortcut).to_owned())
                    .collect()
            },
            available: action.available,
        })
        .collect::<Vec<_>>();
    rows.push(ShortcutRow {
        label: "Keyboard shortcuts".into(),
        shortcuts: if state.herdr_prefix {
            labels(Command::Keybinds)
        } else {
            vec!["⌘/".into()]
        },
        available: actions::catalog_context_available(terminal),
    });
    rows
}

pub(crate) const fn terminal_priority(values: &Values) -> bool {
    values.terminal_priority
}

/// Edits the setting already read by `actions::shortcut`.
pub(crate) fn set_terminal_priority(values: &mut Values, enabled: bool) {
    values.terminal_priority = enabled;
}
