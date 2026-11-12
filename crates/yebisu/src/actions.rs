//! P7's single catalog view, shared by shortcuts, buttons and the palette.
//! Native menu and Command-hold metadata also derive from this catalog.
use crate::{
    app::Smoke,
    board::{Navigation, Target, snapshot},
    terminal_view::TerminalView,
};
use gpui::Keystroke;
use sheprd_commands::{Command, CommandInfo};
use std::collections::BTreeSet;

#[derive(Default)]
pub(crate) struct Actions {
    pub board_hidden: bool,
    pub palette: bool,
    pub terminal_priority: bool,
    pub consumed: BTreeSet<String>,
}
pub(crate) struct Action {
    pub info: &'static CommandInfo,
    pub label: String,
    pub shortcuts: &'static [&'static str],
    pub available: bool,
}
fn supported(command: Command) -> bool {
    use Command::*;
    matches!(
        command,
        Tab | SplitRight
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
pub(crate) fn catalog(terminal: &TerminalView) -> Vec<Action> {
    sheprd_commands::COMMANDS
        .iter()
        .filter(|i| supported(i.command))
        .map(|info| {
            let label = crate::i18n::command(info);
            let available = local(info.command)
                || (terminal.live.connected
                    && !terminal.reconnect.suspended
                    && terminal.navigating.is_none()
                    && terminal
                        .live
                        .surface
                        .as_ref()
                        .is_none_or(|s| s.popup.is_none())
                    && snapshot(&terminal.live)
                        .and_then(|s| sheprd_commands::request(info.command, s))
                        .is_some_and(|(method, _)| {
                            terminal.live.methods.iter().any(|m| m == method.as_str())
                        }));
            Action {
                info,
                label,
                shortcuts: info.default_shortcuts(),
                // Core has its own controls and does not render the palette.
                available: available && !terminal.core_mode,
            }
        })
        .collect()
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
    if !terminal.open
        || !terminal.input.composition.text.is_empty()
        || terminal
            .live
            .surface
            .as_ref()
            .is_some_and(|s| s.popup.is_some())
        || (actions.terminal_priority && terminal.input.options.command_control)
    {
        return None;
    }
    catalog(terminal)
        .into_iter()
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
    pub(crate) fn run_action(&mut self, command: Command) -> crate::Result<()> {
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
                self.actions.board_hidden ^= true;
                return Ok(());
            }
            Command::Palette => {
                self.actions.palette ^= true;
                self.terminal.input.detach();
                return Ok(());
            }
            _ => {}
        }
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host state busy; try again".into()))?;
        if !live.connected
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
