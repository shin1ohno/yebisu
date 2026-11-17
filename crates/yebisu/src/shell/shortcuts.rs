//! L6: the Shortcuts popover. Owner: L6.
//!
//! The status bar's Shortcuts opens a 760 pt popover above it (plan v2
//! 7.11): the action catalog's rows in 3 columns (1 below 760 pt), 36 pt
//! each, a tap runs the row, an unavailable row is disabled. Below the rows
//! it shows the current Command key and Shortcut priority. It replaces the
//! command palette as the status bar's target; `Command::Palette` stays a
//! local catalog command (P12QA, the OS menu and the legacy actions band).
use super::{ChromeState, EntryAction, EntryPoint, HerdrKind, Place, host_menus};
use crate::{
    actions::{self, Action},
    app::Smoke,
    native_menus::shortcut_glyphs,
    terminal_view::TerminalView,
};
use gpui::Context;
use sheprd_commands::Command;

mod view;

/// The popover's width, and the popover width below which it has 1 column
/// (design text D1: min(760, window − 32)).
pub(crate) const POPOVER_WIDTH: f32 = 760.;
pub(crate) const ROW_HEIGHT: f32 = 36.;
const POPOVER_MARGIN: f32 = 16.;
const WIDE_COLUMNS: u8 = 3;

/// The canvas' row groups (`Shortcuts.dc.html`), one per column at 3
/// columns. The catalog's order is kept inside a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutGroup {
    Tabs,
    Panes,
    App,
}

impl ShortcutGroup {
    pub(crate) const ALL: [Self; 3] = [Self::Tabs, Self::Panes, Self::App];
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Tabs => "TABS",
            Self::Panes => "PANES",
            Self::App => "APP",
        }
    }
    pub(crate) fn of(command: Command) -> Self {
        match command {
            Command::Tab
            | Command::NextTab
            | Command::PreviousTab
            | Command::CloseTab
            | Command::TabNumber(_) => Self::Tabs,
            Command::SplitRight
            | Command::SplitDown
            | Command::FocusLeft
            | Command::FocusRight
            | Command::FocusUp
            | Command::FocusDown
            | Command::NextPane
            | Command::PreviousPane
            | Command::Zoom
            | Command::ClosePane => Self::Panes,
            _ => Self::App,
        }
    }
}

/// `rows` split into the non-empty groups, in group order.
pub(crate) fn grouped(
    rows: Vec<(Command, ShortcutRow)>,
) -> Vec<(ShortcutGroup, Vec<(Command, ShortcutRow)>)> {
    ShortcutGroup::ALL
        .into_iter()
        .map(|group| {
            let members = rows
                .iter()
                .filter(|(command, _)| ShortcutGroup::of(*command) == group)
                .cloned()
                .collect::<Vec<_>>();
            (group, members)
        })
        .filter(|(_, members)| !members.is_empty())
        .collect()
}

#[derive(Debug, Default)]
pub(crate) struct ShortcutsState {
    pub(crate) open: bool,
    /// The host the last Host › Edit pick asked to edit (L7 reads it).
    pub(crate) edit_host: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutRow {
    pub(crate) label: String,
    pub(crate) keys: String,
    pub(crate) enabled: bool,
}

/// Whether the catalog lists `command` in the popover. The palette is the
/// popover's predecessor, so it is not a row of it.
fn in_popover(action: &Action) -> bool {
    action.info.command != Command::Palette
}

/// The chrome's part of a row's availability: a sheet blocks every row,
/// local commands need nothing else, Find needs `find::available`, and an
/// RPC command needs a connected Herdr 1 (the catalog is Herdr 1's).
fn chrome_allows(command: Command, state: &ChromeState) -> bool {
    if state.sheet_open {
        return false;
    }
    match command {
        Command::ToggleSidebar | Command::Palette => true,
        Command::Find => state.find_available,
        _ => state.connected && state.kind == HerdrKind::Herdr1,
    }
}

/// Why the chrome disables `command`'s row (q4: a disabled place says why).
fn chrome_reason(command: Command, state: &ChromeState) -> Option<&'static str> {
    if chrome_allows(command, state) {
        None
    } else if state.sheet_open {
        Some("Close the sheet first")
    } else if command == Command::Find {
        Some("Find is not available here")
    } else if state.kind != HerdrKind::Herdr1 {
        Some("Not available on Herdr 2")
    } else {
        Some("Host is not connected")
    }
}

fn rows_from(catalog: Vec<Action>, state: &ChromeState, live: bool) -> Vec<(Command, ShortcutRow)> {
    catalog
        .into_iter()
        .filter(in_popover)
        .map(|action| {
            let command = action.info.command;
            let enabled = chrome_allows(command, state) && (!live || action.available);
            let row = ShortcutRow {
                label: action.label,
                keys: shortcut_glyphs(action.shortcuts),
                enabled,
            };
            (command, row)
        })
        .collect()
}

/// Rows from the action catalog (`actions::catalog`), enabled from `state`.
pub(crate) fn shortcut_rows(state: &ChromeState) -> Vec<ShortcutRow> {
    rows_from(actions::catalog(&TerminalView::default()), state, false)
        .into_iter()
        .map(|(_, row)| row)
        .collect()
}

/// The rows the popover draws for `terminal`: the catalog's own
/// availability and the chrome's must both allow a row.
pub(crate) fn popover_rows(
    terminal: &TerminalView,
    state: &ChromeState,
) -> Vec<(Command, ShortcutRow)> {
    rows_from(actions::catalog(terminal), state, true)
}

/// 3 columns at 760 pt and wider, 1 below.
pub(crate) fn popover_columns(width: f32) -> u8 {
    if width >= POPOVER_WIDTH {
        WIDE_COLUMNS
    } else {
        1
    }
}

/// The popover's width in a window `width` wide: 760 pt, or the window
/// less its margins, so it never leaves the screen. Its columns follow this
/// width: `popover_columns(popover_width(window))`.
pub(crate) fn popover_width(width: f32) -> f32 {
    POPOVER_WIDTH.min((width - 2. * POPOVER_MARGIN).max(0.))
}

fn entry_action(command: Command) -> Option<EntryAction> {
    match command {
        Command::Tab => Some(EntryAction::NewTab),
        Command::SplitRight | Command::SplitDown => Some(EntryAction::Split),
        Command::Find => Some(EntryAction::Find),
        Command::ToggleSidebar => Some(EntryAction::ToggleSidebar),
        _ => None,
    }
}

fn point(
    action: EntryAction,
    place: Place,
    enabled: bool,
    reason: Option<&'static str>,
) -> EntryPoint {
    EntryPoint {
        action,
        place,
        enabled,
        reason,
    }
}

/// L6's entry points: Shortcuts in the status bar, the popover's rows, and
/// the Host / Space items of the menu bar and of long press.
pub(crate) fn shortcuts_entry_points(state: &ChromeState) -> Vec<EntryPoint> {
    let mut out = vec![point(EntryAction::Shortcuts, Place::StatusBar, true, None)];
    let catalog = actions::catalog(&TerminalView::default());
    for (command, row) in rows_from(catalog, state, false) {
        if let Some(action) = entry_action(command) {
            let reason = chrome_reason(command, state);
            out.push(point(action, Place::ShortcutsPopover, row.enabled, reason));
        }
    }
    let menus = host_menus::menu_bar(state, true);
    let long_press = host_menus::host_long_press(state)
        .into_iter()
        .chain(host_menus::space_long_press(state));
    let listed = menus
        .into_iter()
        .flat_map(|(_, entries)| entries)
        .map(|entry| (Place::MenuBar, entry))
        .chain(long_press.map(|entry| (Place::LongPress, entry)));
    for (place, entry) in listed {
        let action = match entry.item {
            host_menus::MenuItemId::Host(host_menus::HostMenuItem::Reconnect) => {
                EntryAction::Reconnect
            }
            host_menus::MenuItemId::Host(host_menus::HostMenuItem::Edit) => EntryAction::Hosts,
            host_menus::MenuItemId::Space(host_menus::SpaceMenuItem::NewTab) => EntryAction::NewTab,
            _ => continue,
        };
        out.push(point(action, place, entry.enabled, entry.reason));
    }
    out
}

impl Smoke {
    /// The chrome state the popover's rows read: the frame's (L2a), with
    /// the Hosts sheet (L7) counted as a sheet, since it is modal too.
    pub(crate) fn popover_chrome_state(&self) -> ChromeState {
        let mut state = self.chrome_state();
        state.sheet_open |= self.hosts_sheet_modal();
        state
    }

    /// Status bar Shortcuts. `false` means not handled: the caller falls
    /// back to the command palette (plan v2 5.2, L2a column).
    pub(crate) fn open_shortcuts(&mut self, cx: &mut Context<Self>) -> bool {
        if self.settings_modal || self.hosts_sheet_modal() {
            return false;
        }
        self.shell.shortcuts.open ^= true;
        self.actions.palette = false;
        cx.notify();
        true
    }
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;
