//! Rows come from the catalog; unavailable rows are disabled.
use super::super::*;
use super::support::{connected, terminal};
use crate::actions;
use sheprd_commands::Command;

#[test]
fn shortcuts_rows_come_from_catalog() {
    let catalog = actions::catalog(&TerminalView::default());
    let rows = shortcut_rows(&connected());
    assert_eq!(rows.len(), catalog.len());
    for (row, action) in rows.iter().zip(&catalog) {
        assert_eq!(row.label, action.label);
        assert_eq!(row.keys, shortcut_glyphs(action.shortcuts));
    }
    // Every catalog command has a row, the palette too (the canvas lists it).
    let palette = catalog
        .iter()
        .find(|row| row.info.command == Command::Palette)
        .map(|row| row.label.clone());
    assert!(palette.is_some_and(|label| rows.iter().any(|row| row.label == label)));
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

#[gpui::test]
fn popover_reads_find_and_sheets_from_the_frame(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext;
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.terminal = terminal(&["pane.copy_search"]);
            // The popover takes Find's availability from the frame (L2a).
            let frame = this.chrome_state();
            assert_eq!(
                this.popover_chrome_state().find_available,
                frame.find_available
            );
            assert!(!this.popover_chrome_state().sheet_open);
            // The Hosts sheet counts as a sheet, as Settings does.
            assert!(this.open_hosts(cx));
            assert!(this.popover_chrome_state().sheet_open);
        })
    });
}
