//! L6: the Shortcuts popover. Owner: L6.
//!
//! The status bar's Shortcuts opens a 760 pt popover above it (plan v2
//! 7.11): the action catalog's rows in 3 columns (1 below 760 pt), 36 pt
//! each, a tap runs the row, an unavailable row is disabled. Below the rows
//! it shows the current Command key and Shortcut priority. It replaces the
//! command palette as the status bar's target; `Command::Palette` stays a
//! local catalog command (P12QA, the OS menu and the legacy actions band)
//! and, as the canvas shows, a row of the popover that opens the palette.
use super::{ChromeState, EntryAction, EntryPoint, HerdrKind, Place, host_menus};
use crate::{
    actions::{self, Action},
    app::Smoke,
    native_menus::shortcut_glyphs,
    terminal_view::TerminalView,
};
use gpui::{Context, Window};
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum PopoverAction {
    Catalog(Command),
    Focus(ShortcutGroup),
    Copy,
    Paste,
}

fn is_grouped_focus(command: Command) -> bool {
    matches!(
        command,
        Command::TabNumber(_)
            | Command::FocusLeft
            | Command::FocusRight
            | Command::FocusUp
            | Command::FocusDown
    )
}

fn canvas_rows(
    group: ShortcutGroup,
    rows: Vec<(Command, ShortcutRow)>,
    expanded: bool,
) -> Vec<(PopoverAction, ShortcutRow)> {
    let mut shown = Vec::new();
    let mut focus_inserted = false;
    let focus_enabled = rows
        .iter()
        .any(|(command, row)| is_grouped_focus(*command) && row.enabled);
    for (command, row) in rows {
        if group == ShortcutGroup::App
            && !matches!(command, Command::ToggleSidebar | Command::Palette)
        {
            continue;
        }
        if is_grouped_focus(command) {
            if !focus_inserted {
                let (label, keys) = if group == ShortcutGroup::Tabs {
                    ("Focus Tab 1–9", "⌘1 … ⌘9")
                } else {
                    ("Focus Left · Right · Up · Down", "⌥⌘←→↑↓")
                };
                shown.push((
                    PopoverAction::Focus(group),
                    ShortcutRow {
                        label: label.into(),
                        keys: keys.into(),
                        enabled: focus_enabled,
                    },
                ));
                focus_inserted = true;
            }
            if !expanded {
                continue;
            }
        }
        shown.push((PopoverAction::Catalog(command), row));
    }
    shown
}

fn show_hold_hint(hardware_keyboard: bool) -> Option<&'static str> {
    hardware_keyboard.then_some(view::HINT)
}

#[derive(Debug, Default)]
pub(crate) struct ShortcutsState {
    pub(crate) open: bool,
    /// Transient popovers opened by the tab row and the status bar.
    pub(super) space_picker_open: bool,
    pub(super) status_detail_open: bool,
    expanded: Option<ShortcutGroup>,
    /// The host the last Host › Edit pick asked to edit (L7 reads it).
    pub(crate) edit_host: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutRow {
    pub(crate) label: String,
    pub(crate) keys: String,
    pub(crate) enabled: bool,
}

/// The chrome's part of a row's availability: a sheet blocks every row,
/// local commands need nothing else, Find needs `find::available`, and an
/// RPC command needs a connected Herdr 1 (the catalog is Herdr 1's).
fn chrome_allows(command: Command, state: &ChromeState) -> bool {
    if state.sheet_open {
        return false;
    }
    match command {
        // While the width forces the rail the toggle cannot change anything
        // (Integration 2, L4 review F1).
        Command::ToggleSidebar => !state.sidebar_forced,
        Command::Palette => true,
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
    } else if command == Command::ToggleSidebar {
        super::rail::toggle_unavailable(state)
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
    // While a sheet is open the status bar's Shortcuts does nothing (S7).
    let sheet = state.sheet_open.then_some("Close the sheet first");
    let mut out = vec![point(
        EntryAction::Shortcuts,
        Place::StatusBar,
        !state.sheet_open,
        sheet,
    )];
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
    /// back to the command palette (plan v2 5.2, L2a column). Over a sheet
    /// it is handled and does nothing, so the palette does not open under
    /// the sheet (review S7).
    pub(crate) fn open_shortcuts(&mut self, cx: &mut Context<Self>) -> bool {
        if self.settings_modal || self.hosts_sheet_modal() {
            return true;
        }
        self.shell.shortcuts.open ^= true;
        if self.shell.shortcuts.open {
            // The popover and the palette exclude each other (S6); the
            // terminal stops taking text while it is open (S1, the
            // L6 part; the key guards are integrator patch 05).
            self.actions.palette = false;
            self.terminal.input.detach();
            // As opening Settings or the Hosts sheet does (Integration 2
            // review adf0 SF1): no selection or handoff stamp stays behind.
            self.handoff.sync(None);
            self.handoff.cancel();
            self.handoff.select_mode = false;
        }
        cx.notify();
        true
    }

    /// The status bar's Shortcuts: opens the popover, or closes it and gives
    /// the terminal its keyboard back.
    pub(crate) fn toggle_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.shell.shortcuts.open {
            self.close_shortcuts(window, cx);
            return true;
        }
        self.open_shortcuts(cx)
    }

    /// Closes the popover by the background or the Shortcuts toggle. The
    /// terminal gets the focus, the keyboard and text input back as after
    /// closing Settings (`end_settings_modal`) or the Hosts sheet
    /// (`return_keys_to_terminal`); otherwise keys typed without touching
    /// the terminal are lost (Integration 2 review adf0 SF1).
    pub(crate) fn close_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shell.shortcuts.open = false;
        self.shell.shortcuts.expanded = None;
        self.refresh_terminal_input();
        if self.terminal.open && self.terminal.input.ready {
            self.terminal_focus.focus(window, cx);
            self.request_terminal_keyboard(window, "shortcuts-close");
            self.request_text_retirement();
        }
        window.notify_text_input_changed(&self.terminal_focus);
        cx.notify();
    }

    /// Whether the popover stays open this frame. A sheet or the palette
    /// closes it: they exclude each other both ways (S6).
    pub(crate) fn shortcuts_shown(&mut self) -> bool {
        if self.settings_modal || self.hosts_sheet_modal() || self.actions.palette {
            self.shell.shortcuts.open = false;
        }
        self.shell.shortcuts.open
    }

    /// A tap on the popover's `command` row. A row the popover shows
    /// disabled runs nothing (`false`). Find opens the find bar as the
    /// legacy button does (`action_view.rs`), since it is not an RPC
    /// command (review B1); the others go through `run_action`.
    pub(crate) fn tap_shortcut_row(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let enabled = popover_rows(&self.terminal, &self.popover_chrome_state())
            .into_iter()
            .any(|(c, row)| c == command && row.enabled);
        if !self.shortcuts_shown() || !enabled {
            return false;
        }
        self.shell.shortcuts.open = false;
        if command == Command::Find {
            self.open_find(window, cx);
            cx.notify();
            return true;
        }
        if let Err(e) = self.run_action(command) {
            self.terminal.live.status = e.to_string();
        }
        window.notify_text_input_changed(&self.terminal_focus);
        window.dismiss_virtual_keyboard();
        cx.notify();
        true
    }
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;
