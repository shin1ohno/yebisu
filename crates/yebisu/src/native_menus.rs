//! Keyless UIKit commands: labels advertise shortcuts without registering keys.
//! Native stale tags are discarded by the pinned SDK before action dispatch.
use crate::{actions, app::Smoke, board::snapshot, terminal_view::TerminalView};
use gpui::{Context, Menu, MenuItem, Window};
use sheprd_commands::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Scope {
    host: String,
    generation: u64,
    boot: String,
    workspace: Option<String>,
    tab: Option<String>,
    pane: Option<String>,
    epoch: u64,
    methods: Vec<String>,
    palette: bool,
    focused: bool,
    availability: Vec<bool>,
}
impl Scope {
    fn current(terminal: &TerminalView, actions: &actions::Actions, focused: bool) -> Self {
        let s = snapshot(&terminal.live);
        Self {
            host: terminal.reconnect.target.clone(),
            generation: terminal.link.generation,
            boot: s.map(|s| s.boot_id.clone()).unwrap_or_default(),
            workspace: s.and_then(|s| s.focused_workspace_id.clone()),
            tab: s.and_then(|s| s.focused_tab_id.clone()),
            pane: s.and_then(|s| s.focused_pane_id.clone()),
            epoch: terminal.live.input_epoch,
            methods: terminal.live.methods.clone(),
            palette: actions.palette,
            focused,
            availability: catalog_rows(terminal, actions, focused)
                .iter()
                .map(|row| row.available)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(namespace = yebisu, no_json)]
pub(crate) struct NativeCommand {
    name: String,
}

/// Shared Settings/menu presentation API. No platform keymap is modified.
/// The Settings sheet supplies its intended terminal focus/scene context.
pub(crate) fn catalog_rows(
    terminal: &TerminalView,
    actions: &actions::Actions,
    focused: bool,
) -> Vec<actions::Action> {
    actions::catalog(terminal)
        .into_iter()
        .map(|mut row| {
            row.available &=
                focused && !(actions.terminal_priority && terminal.input.options.command_control);
            row
        })
        .collect()
}

fn menu_label(row: &actions::Action) -> String {
    let shortcuts = row
        .shortcuts
        .iter()
        .map(|shortcut| {
            shortcut
                .replace("cmd-", "⌘")
                .replace("ctrl-", "⌃")
                .replace("alt-", "⌥")
                .replace("shift-", "⇧")
                .to_uppercase()
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if shortcuts.is_empty() {
        row.label.clone()
    } else {
        format!("{}　{shortcuts}", row.label)
    }
}

#[derive(Default)]
pub(crate) struct NativeMenus {
    scope: Option<Scope>,
    rows: Vec<(String, String, bool)>,
}
impl NativeMenus {
    fn refresh(
        &mut self,
        terminal: &TerminalView,
        actions: &actions::Actions,
        focused: bool,
    ) -> bool {
        let scope = Scope::current(terminal, actions, focused);
        let rows = catalog_rows(terminal, actions, focused)
            .iter()
            .map(|row| (row.info.name.to_owned(), menu_label(row), row.available))
            .collect();
        if self.scope.as_ref() == Some(&scope) && self.rows == rows {
            return false;
        }
        self.scope = Some(scope);
        self.rows = rows;
        true
    }

    fn command_gate(
        &self,
        command: Command,
        installed: bool,
        terminal: &TerminalView,
        actions: &actions::Actions,
        focused: bool,
    ) -> crate::Result<()> {
        if !focused {
            return Err(crate::Error::NativeMenu("inactive terminal focus"));
        }
        // Approach C: only a menu selection carries displayed-scope authority.
        // A physical key is a new intent and uses the current catalog state.
        if installed && self.scope.as_ref() != Some(&Scope::current(terminal, actions, focused)) {
            return Err(crate::Error::NativeMenu("displayed menu scope changed"));
        }
        if !catalog_rows(terminal, actions, focused)
            .iter()
            .any(|row| row.info.command == command && row.available)
        {
            return Err(crate::Error::NativeMenu("command unavailable"));
        }
        Ok(())
    }
}

impl Smoke {
    pub(crate) fn run_catalog_command(
        &mut self,
        command: Command,
        menu: Option<&NativeCommand>,
        focused: bool,
    ) -> crate::Result<()> {
        self.native_menus.command_gate(
            command,
            menu.is_some(),
            &self.terminal,
            &self.actions,
            focused,
        )?;
        self.run_action(command)
    }

    pub(crate) fn install_native_menus(&mut self, window: &Window, cx: &mut Context<Self>) {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if self.p12_qa.freeze {
            return;
        }
        let focused = self.terminal_focus.is_focused(window) && window.is_window_active();
        if !self
            .native_menus
            .refresh(&self.terminal, &self.actions, focused)
        {
            return;
        }
        let items = self.native_menus.rows.iter().map(|(name, label, enabled)| {
            MenuItem::action(label.clone(), NativeCommand { name: name.clone() }).disabled(!enabled)
        });
        cx.set_menus(vec![Menu::new("Terminal").items(items)]);
    }

    pub(crate) fn native_menu_action(
        &mut self,
        action: &NativeCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused = self.terminal_focus.is_focused(window) && window.is_window_active();
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let receipt = self.p12_menu_begin(&action.name, "native_action", window);
        let result = actions::catalog(&self.terminal)
            .iter()
            .find(|row| row.info.name == action.name)
            .map(|row| row.info.command)
            .ok_or(crate::Error::NativeMenu("unsupported command"))
            .and_then(|command| self.run_catalog_command(command, Some(action), focused));
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.p12_menu_end(Some(receipt), result.is_ok(), "command_gate");
        self.observation.record(serde_json::json!({
            "kind": "native-menu", "name": action.name, "entry": "action",
            "accepted": result.is_ok(), "error": result.as_ref().err().map(ToString::to_string),
        }));
        if let Err(error) = result {
            self.terminal.live.status = error.to_string();
        }
        cx.stop_propagation();
        window.notify_text_input_changed(&self.terminal_focus);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "native_menus_tests.rs"]
mod tests;
