//! Read-only supported shortcut catalog and the persisted terminal priority.

use crate::{
    actions::{self, Actions, HELP_SHORTCUT, SETTINGS_SHORTCUT},
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
    let labels = |command| state.host_shortcut_labels(terminal, command);
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
            vec![HELP_SHORTCUT.into()]
        },
        available: actions::catalog_context_available(terminal),
    });
    rows.extend([
        ShortcutRow {
            label: "Settings".into(),
            shortcuts: vec![SETTINGS_SHORTCUT.into()],
            available: true,
        },
        ShortcutRow {
            label: "Copy".into(),
            shortcuts: vec!["⌘C".into()],
            available: true,
        },
        ShortcutRow {
            label: "Paste".into(),
            shortcuts: vec!["⌘V".into()],
            available: true,
        },
    ]);
    rows
}

impl ShortcutRow {
    pub(crate) fn display(&self, herdr_prefix: bool) -> String {
        let keys = if self.shortcuts.is_empty() {
            "—".into()
        } else {
            self.shortcuts.join(if herdr_prefix { " / " } else { ", " })
        };
        let availability = if self.available { "" } else { " (unavailable)" };
        format!("{} · {keys}{availability}", self.label)
    }
}

pub(crate) const fn terminal_priority(values: &Values) -> bool {
    values.terminal_priority
}

/// Edits the setting already read by `actions::shortcut`.
pub(crate) fn set_terminal_priority(values: &mut Values, enabled: bool) {
    values.terminal_priority = enabled;
}

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
