//! Rows come from the catalog; unavailable rows are disabled.
use super::super::*;
use super::support::{connected, terminal};
use crate::actions;
use sheprd_commands::Command;

#[test]
fn shortcuts_rows_come_from_catalog() {
    let catalog: Vec<_> = actions::catalog(&TerminalView::default())
        .into_iter()
        .filter(|row| row.info.command != Command::Palette)
        .collect();
    let rows = shortcut_rows(&connected());
    assert_eq!(rows.len(), catalog.len());
    for (row, action) in rows.iter().zip(&catalog) {
        assert_eq!(row.label, action.label);
        assert_eq!(row.keys, shortcut_glyphs(action.shortcuts));
    }
    // Every catalog command but the palette has a row, in catalog order.
    let all = actions::catalog(&TerminalView::default());
    assert_eq!(rows.len() + 1, all.len());
    let palette = all
        .iter()
        .find(|row| row.info.command == Command::Palette)
        .map(|row| row.label.clone());
    assert!(palette.is_some_and(|label| rows.iter().all(|row| row.label != label)));
    // The popover of a live terminal lists the same rows.
    let live = popover_rows(&terminal(&["tab.create"]), &connected());
    let live: Vec<_> = live
        .into_iter()
        .map(|(_, row)| (row.label, row.keys))
        .collect();
    let pure: Vec<_> = rows.into_iter().map(|row| (row.label, row.keys)).collect();
    assert_eq!(live, pure);
    // A shortcut shows as menu glyphs.
    let split = actions::catalog(&TerminalView::default())
        .into_iter()
        .find(|row| row.info.command == Command::SplitRight)
        .and_then(|row| row.shortcuts.first().copied());
    assert!(split.is_some_and(|key| !shortcut_glyphs_of(key).contains('-')));
}

fn shortcut_glyphs_of(key: &str) -> String {
    shortcut_glyphs(&[key])
}

fn enabled(rows: &[(Command, ShortcutRow)], command: Command) -> bool {
    rows.iter()
        .find(|(c, _)| *c == command)
        .is_some_and(|(_, row)| row.enabled)
}

#[test]
fn unavailable_rows_are_disabled() {
    let label = |command: Command| {
        actions::catalog(&TerminalView::default())
            .into_iter()
            .find(|row| row.info.command == command)
            .map(|row| row.label)
            .unwrap_or_default()
    };
    let row_enabled = |state: &ChromeState, command: Command| {
        shortcut_rows(state)
            .into_iter()
            .find(|row| row.label == label(command))
            .is_some_and(|row| row.enabled)
    };
    // Connected Herdr 1: RPC rows, local rows and Find are enabled.
    let state = connected();
    assert!(row_enabled(&state, Command::SplitRight));
    assert!(row_enabled(&state, Command::ToggleSidebar));
    assert!(row_enabled(&state, Command::Find));
    // Disconnected: RPC rows are disabled, local rows stay enabled.
    let offline = ChromeState {
        connected: false,
        ..state
    };
    assert!(!row_enabled(&offline, Command::SplitRight));
    assert!(!row_enabled(&offline, Command::Tab));
    assert!(row_enabled(&offline, Command::ToggleSidebar));
    // Find follows `find::available`.
    let no_find = ChromeState {
        find_available: false,
        ..state
    };
    assert!(!row_enabled(&no_find, Command::Find));
    // Herdr 2 has no catalog RPC commands.
    let herdr2 = ChromeState {
        kind: HerdrKind::Herdr2,
        ..state
    };
    assert!(!row_enabled(&herdr2, Command::SplitRight));
    // An open sheet disables every row.
    let sheet = ChromeState {
        sheet_open: true,
        ..state
    };
    assert!(shortcut_rows(&sheet).iter().all(|row| !row.enabled));
    // A live terminal: a command whose method the host lacks is disabled
    // even when the chrome allows it.
    let rows = popover_rows(&terminal(&["tab.create", "tab.focus"]), &state);
    assert!(enabled(&rows, Command::Tab));
    assert!(!enabled(&rows, Command::SplitRight));
    let rows = popover_rows(
        &terminal(&["tab.create", "tab.focus", "pane.split"]),
        &state,
    );
    assert!(enabled(&rows, Command::SplitRight));
}
