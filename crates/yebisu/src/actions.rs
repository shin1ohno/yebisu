//! P7's single catalog view, shared by shortcuts, buttons and the palette.
//! Keyless native menus and Settings labels also derive from this catalog.
pub(crate) mod herdr_keys;

use crate::{
    app::Smoke,
    board::{Navigation, Target, snapshot},
    core::{Command as CoreCommand, SessionInfo, TabInfo},
    terminal_view::TerminalView,
};
use gpui::Keystroke;
use sheprd_commands::{Command, CommandInfo};
use std::collections::BTreeSet;

pub(crate) struct Actions {
    pub board_hidden: bool,
    pub palette: bool,
    pub terminal_priority: bool,
    pub consumed: BTreeSet<String>,
    pub herdr_prefix: bool,
    pub use_server_keybindings: bool,
    pub host_keys: herdr_keys::HostKeys,
}
impl Default for Actions {
    fn default() -> Self {
        Self {
            board_hidden: false,
            palette: false,
            terminal_priority: false,
            consumed: BTreeSet::new(),
            herdr_prefix: false,
            use_server_keybindings: true,
            host_keys: herdr_keys::HostKeys::default(),
        }
    }
}
impl Actions {
    pub(crate) fn host_shortcut_labels(
        &self,
        terminal: &TerminalView,
        command: Command,
    ) -> Vec<String> {
        if terminal.core_mode && command != Command::Keybinds && !core_recognizes(command) {
            return Vec::new();
        }
        self.host_keys
            .labels(terminal, self.use_server_keybindings, command)
    }

    pub(crate) fn first_shortcut(
        &self,
        terminal: &TerminalView,
        command: Command,
        ipad: &'static str,
    ) -> Option<String> {
        if self.herdr_prefix {
            self.host_shortcut_labels(terminal, command)
                .into_iter()
                .next()
        } else {
            Some(ipad.into())
        }
    }
}

pub(crate) const SETTINGS_SHORTCUT: &str = "⌘,";
pub(crate) const HELP_SHORTCUT: &str = "⌘/";

pub(crate) struct Action {
    pub info: &'static CommandInfo,
    pub label: String,
    pub shortcuts: &'static [&'static str],
    pub available: bool,
}
pub(crate) fn supported(command: Command) -> bool {
    use Command::*;
    matches!(
        command,
        Find | Tab
            // New space (plan v2 7.6): a Herdr 1 workspace, from the footer.
            | Workspace
            | SplitRight
            | SplitDown
            | NextTab
            | PreviousTab
            | FocusLeft
            | FocusRight
            | FocusUp
            | FocusDown
            | NextPane
            | PreviousPane
            | Zoom
            | ClosePane
            | CloseTab
            | TabNumber(1..=9)
            | ToggleSidebar
            | Palette
    )
}
fn local(command: Command) -> bool {
    matches!(command, Command::ToggleSidebar | Command::Palette)
}
// Shared command context, independent of whether a command needs an RPC.
// Focus/scene and keyboard priority remain entry-point policy, not transport state.
pub(crate) fn catalog_context_available(terminal: &TerminalView) -> bool {
    terminal.open
        && (!terminal.core_mode || terminal.core.is_some())
        && terminal
            .live
            .surface
            .as_ref()
            .is_none_or(|s| s.popup.is_none())
        && terminal.input.composition.text.is_empty()
}
pub(crate) fn catalog(terminal: &TerminalView) -> Vec<Action> {
    let context_available = catalog_context_available(terminal);
    let core = current_core_session(terminal);
    sheprd_commands::COMMANDS
        .iter()
        .filter(|i| supported(i.command))
        .map(|info| {
            let label = crate::i18n::command(info);
            let available = if info.command == Command::Find {
                context_available && crate::find_view::available(terminal)
            } else if terminal.core_mode {
                context_available
                    && (local(info.command)
                        || (terminal.live.connected
                            && !terminal.reconnect.suspended
                            && core.as_ref().is_some_and(|session| {
                                core_action(info.command, session).is_some()
                            })))
            } else {
                context_available
                    && (local(info.command)
                        || (terminal.live.connected
                            && !terminal.reconnect.suspended
                            && terminal.live.version_mismatch.get().is_none()
                            && terminal.navigating.is_none()
                            && snapshot(&terminal.live)
                                .and_then(|s| sheprd_commands::request(info.command, s))
                                .is_some_and(|(method, _)| {
                                    terminal.live.methods.iter().any(|m| m == method.as_str())
                                })))
            };
            Action {
                info,
                label,
                shortcuts: info.default_shortcuts(),
                available,
            }
        })
        .collect()
}
fn core_recognizes(command: Command) -> bool {
    use Command::*;
    matches!(
        command,
        Tab | CloseTab
            | SplitRight
            | SplitDown
            | TabNumber(1..=9)
            | NextTab
            | PreviousTab
            | FocusLeft
            | FocusRight
            | FocusUp
            | FocusDown
            | NextPane
            | PreviousPane
            | ToggleSidebar
            | Palette
    )
}

pub(crate) fn current_core_session(terminal: &TerminalView) -> Option<SessionInfo> {
    let core = terminal.core.as_ref()?;
    core.sessions(terminal.live.input_epoch)
        .into_iter()
        .find(|row| row.name == core.name())
}

fn core_action(command: Command, session: &SessionInfo) -> Option<CoreCommand> {
    use Command::*;
    let selected = session.tabs.iter().position(|tab| tab.active);
    let current = selected.and_then(|index| session.tabs.get(index));
    let select = |index: usize| {
        session
            .tabs
            .get(index)
            .map(|tab| CoreCommand::SelectTab(tab.id.clone()))
    };
    match command {
        Tab => Some(CoreCommand::NewTab),
        CloseTab => current.map(|tab| CoreCommand::CloseTab(tab.id.clone())),
        SplitRight | SplitDown if current.is_some_and(|tab| tab.panes.len() == 1) => {
            Some(CoreCommand::SplitInDirection(command == SplitDown))
        }
        TabNumber(number @ 1..=9) => select((number - 1) as usize),
        NextTab if session.tabs.len() > 1 => select((selected? + 1) % session.tabs.len()),
        PreviousTab if session.tabs.len() > 1 => {
            select((selected? + session.tabs.len() - 1) % session.tabs.len())
        }
        NextPane | PreviousPane | FocusLeft | FocusRight | FocusUp | FocusDown => {
            focus_action(command, current?)
        }
        _ => None,
    }
}

fn focus_action(command: Command, tab: &TabInfo) -> Option<CoreCommand> {
    if tab.panes.len() != 2 {
        return None;
    }
    let next = match command {
        Command::NextPane | Command::PreviousPane => 1 - tab.focused_pane,
        Command::FocusLeft if !tab.vertical => 0,
        Command::FocusRight if !tab.vertical => 1,
        Command::FocusUp if tab.vertical => 0,
        Command::FocusDown if tab.vertical => 1,
        _ => return None,
    };
    (next != tab.focused_pane).then(|| CoreCommand::FocusAddress(tab.panes[next].clone()))
}

#[cfg(test)]
pub(crate) fn shortcut(
    terminal: &TerminalView,
    actions: &Actions,
    key: &Keystroke,
) -> Option<Command> {
    shortcut_candidate(terminal, actions, key).filter(|command| {
        catalog(terminal)
            .iter()
            .any(|a| a.info.command == *command && a.available)
    })
}
// Recognition precedes availability: stale native commands must not fall through
// into terminal input when their capability disappears. Policy exclusions still
// belong to the terminal/IME, as before.
pub(crate) fn shortcut_candidate(
    terminal: &TerminalView,
    actions: &Actions,
    key: &Keystroke,
) -> Option<Command> {
    if !catalog_context_available(terminal)
        || (actions.terminal_priority && terminal.input.options.command_control)
    {
        return None;
    }
    if Keystroke::parse("cmd-/")
        .is_ok_and(|help| help.key == key.key && help.modifiers == key.modifiers)
    {
        return Some(Command::Keybinds);
    }
    catalog(terminal)
        .into_iter()
        .filter(|action| !terminal.core_mode || core_recognizes(action.info.command))
        .find(|a| {
            a.shortcuts.iter().any(|s| {
                Keystroke::parse(s).is_ok_and(|mut binding| {
                    normalize_shortcut(&mut binding);
                    binding.key == key.key && binding.modifiers == key.modifiers
                })
            })
        })
        .map(|a| a.info.command)
}
// UIKit names shifted symbols by their resulting US-layout character and
// clears Shift before dispatch (herdr-uikit a573fb3, keymap.rs). Letters and named
// keys retain Shift. Normalize catalog bindings, never incoming text events.
fn normalize_shortcut(binding: &mut Keystroke) {
    if !binding.modifiers.shift {
        return;
    }
    let shifted = match binding.key.as_str() {
        "[" => "{",
        "]" => "}",
        // These are the only shifted-symbol defaults in the supported catalog.
        _ => return,
    };
    binding.key = shifted.into();
    binding.modifiers.shift = false;
}
impl Smoke {
    /// The current Herdr 2 tabs and their stable IDs for the shared tab strip.
    pub(crate) fn core_tabs(&self) -> Vec<TabInfo> {
        current_core_session(&self.terminal).map_or_else(Vec::new, |session| session.tabs)
    }

    /// Focus or close the tab the user actually saw. C2 retains the selected
    /// host and generation from when it rendered the tab.
    pub(crate) fn core_tab_action(
        &mut self,
        host: &str,
        generation: u64,
        id: &str,
        close: bool,
    ) -> crate::Result<()> {
        if host != self.terminal.reconnect.target
            || generation != self.terminal.link.generation
            || !self.terminal.live.connected
            || !self.core_tabs().iter().any(|tab| tab.id == id)
        {
            return Err(crate::Error::Ssh("Stale Herdr 2 tab; tap again".into()));
        }
        self.try_core_command(if close {
            CoreCommand::CloseTab(id.to_owned())
        } else {
            CoreCommand::SelectTab(id.to_owned())
        })
    }

    pub(crate) fn run_action(&mut self, command: Command) -> crate::Result<()> {
        self.input_transition(|this| this.run_action_inner(command))
    }
    fn run_action_inner(&mut self, command: Command) -> crate::Result<()> {
        // A sheet over the terminal (Settings or Hosts, A6) blocks actions.
        if self.input_modal() || self.new_space_open() {
            return Err(crate::Error::Settings(
                "terminal action blocked by settings".into(),
            ));
        }
        self.handoff.cancel();
        self.handoff.selection = None;
        if !catalog(&self.terminal)
            .iter()
            .any(|a| a.info.command == command && a.available)
        {
            return Err(crate::Error::Ssh("Action unavailable".into()));
        }
        match command {
            Command::ToggleSidebar => {
                // Command-B and the palette row would change nothing while
                // the width forces the rail: say why (L4 review F1).
                if let Some(reason) = crate::shell::rail::toggle_unavailable(&self.chrome_state()) {
                    return Err(crate::Error::Settings(reason.into()));
                }
                self.toggle_sidebar();
                return Ok(());
            }
            Command::Palette => {
                self.actions.palette ^= true;
                self.terminal.input.detach();
                return Ok(());
            }
            _ => {}
        }
        if self.terminal.core_mode {
            let action = current_core_session(&self.terminal)
                .and_then(|session| core_action(command, &session))
                .ok_or_else(|| crate::Error::Ssh("Herdr 2 action unavailable".into()))?;
            match action {
                CoreCommand::SelectTab(id) | CoreCommand::CloseTab(id) => {
                    let close = command == Command::CloseTab;
                    let host = self.terminal.reconnect.target.clone();
                    self.core_tab_action(&host, self.terminal.link.generation, &id, close)?;
                }
                _ => self.try_core_command(action)?,
            }
            self.actions.palette = false;
            return Ok(());
        }
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host state busy; try again".into()))?;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || terminal.navigating.is_some()
            || live.surface.as_ref().is_some_and(|s| s.popup.is_some())
        {
            return Err(crate::Error::Ssh(
                "Host unavailable or command pending".into(),
            ));
        }
        let current =
            snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        let shown = snapshot(&terminal.live)
            .ok_or_else(|| crate::Error::Ssh("Waiting for display".into()))?;
        if current.boot_id != shown.boot_id
            || current.focused_workspace_id != shown.focused_workspace_id
            || current.focused_tab_id != shown.focused_tab_id
            || current.focused_pane_id != shown.focused_pane_id
        {
            return Err(crate::Error::Ssh("Focus changed; try again".into()));
        }
        let (method, params) = sheprd_commands::request(command, current)
            .ok_or_else(|| crate::Error::Ssh("Target unavailable".into()))?;
        if !live
            .methods
            .iter()
            .any(|available| available == method.as_str())
        {
            return Err(crate::Error::NativeMenu("capability changed"));
        }
        if !live
            .methods
            .iter()
            .any(|candidate| candidate == method.as_str())
        {
            return Err(crate::Error::NativeMenu("capability changed"));
        }
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .request(&current.boot_id, method, params)
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        terminal.navigating = Some(Navigation {
            boot: current.boot_id.clone(),
            revision: current.revision,
            target: Target::Command,
            request: request.clone(),
        });
        terminal.input.detach();
        live.pending_request = Some(request);
        live.completed_request = None;
        live.failed_request = None;
        self.actions.palette = false;
        self.observation
            .record(serde_json::json!({"kind":"action", "method":method.as_str()}));
        Ok(())
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
