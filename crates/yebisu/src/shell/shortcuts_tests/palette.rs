//! `Command::Palette` stays a local catalog command (PR 44 P12QA).
use super::super::*;
use super::support::terminal;
use crate::{actions, native_menus};
use gpui::{AppContext, TestAppContext};
use sheprd_commands::Command;

#[test]
fn palette_stays_local_command() {
    // Disconnected but open: the palette is still available, like a local
    // command, and the OS menu (P12QA) still lists it.
    let mut t = terminal(&[]);
    t.live.connected = false;
    let palette = actions::catalog(&t)
        .into_iter()
        .find(|row| row.info.command == Command::Palette);
    assert!(palette.is_some_and(|row| row.available));
    let menu = native_menus::catalog_rows(&t, &actions::Actions::default(), true);
    assert!(
        menu.iter()
            .any(|row| row.info.command == Command::Palette && row.available)
    );
    // The popover lists it as a row that is live while disconnected.
    let rows = popover_rows(&t, &ChromeState::default());
    assert!(
        rows.iter()
            .any(|(command, row)| *command == Command::Palette && row.enabled)
    );
}

#[gpui::test]
fn palette_runs_locally_and_shortcuts_opens_the_popover(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.terminal = terminal(&[]);
            this.terminal.live.connected = false;
            assert!(this.run_action(Command::Palette).is_ok());
            assert!(this.actions.palette);
            // Shortcuts opens the popover and closes the palette.
            assert!(this.open_shortcuts(cx));
            assert!(this.shell.shortcuts.open);
            assert!(!this.actions.palette);
            // A second tap closes it; a sheet keeps it from opening.
            assert!(this.open_shortcuts(cx));
            assert!(!this.shell.shortcuts.open);
            // Over a sheet it is handled and does nothing, so the caller
            // does not fall back to the palette under the sheet (S7).
            this.settings_modal = true;
            assert!(this.open_shortcuts(cx));
            assert!(!this.shell.shortcuts.open);
            assert!(!this.actions.palette);
            // So does the Hosts sheet (L7), which is modal too.
            this.settings_modal = false;
            assert!(this.open_hosts(cx));
            assert!(this.open_shortcuts(cx));
            assert!(!this.shell.shortcuts.open);
        })
    });
}

#[gpui::test]
fn popover_and_palette_exclude_each_other(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let view = cx.new(Smoke::new);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.terminal = terminal(&[]);
            // Opening the popover closes the palette...
            assert!(this.run_action(Command::Palette).is_ok());
            assert!(this.open_shortcuts(cx));
            assert!(!this.actions.palette);
            assert!(this.shortcuts_shown());
            // ...and the palette opened from anywhere else closes the popover.
            assert!(this.run_action(Command::Palette).is_ok());
            assert!(!this.shortcuts_shown());
            assert!(!this.shell.shortcuts.open);
        })
    });
}
