//! P12 consumer menus. UIKit owns presentation; the catalog owns command policy.
//! Source: herdr-uikit b2fa781 menus.rs and menu/model.rs (GPUI 0.3.12).
use crate::{actions, app::Smoke, board::snapshot, terminal_view::TerminalView};
use gpui::{Context, KeyBinding, Menu, MenuItem, Window};
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
    enabled: bool,
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
            enabled: focused
                && actions::catalog_context_available(terminal)
                && !(actions.terminal_priority && terminal.input.options.command_control),
        }
    }
}

#[derive(Clone, Debug, gpui::Action)]
#[action(namespace = yebisu, no_json)]
pub(crate) struct NativeCommand {
    name: String,
    revision: Option<u64>,
}
// GPUI uses action equality to find shortcut metadata. Revision is execution
// authority, checked explicitly below, and is deliberately not key identity.
impl PartialEq for NativeCommand {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Default)]
pub(crate) struct NativeMenus {
    scope: Option<Scope>,
    revision: u64,
    consumed: bool,
    bindings_installed: bool,
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
        let rows = actions::catalog(terminal)
            .into_iter()
            .map(|a| {
                (
                    a.info.name.to_owned(),
                    a.label,
                    scope.enabled && a.available,
                )
            })
            .collect::<Vec<_>>();
        if self.scope.as_ref() == Some(&scope) && self.rows == rows && !self.consumed {
            return false;
        }
        self.revision = self.revision.saturating_add(1);
        self.scope = Some(scope);
        self.rows = rows;
        self.consumed = false;
        true
    }
    fn take(
        &mut self,
        action: &NativeCommand,
        terminal: &TerminalView,
        actions: &actions::Actions,
        focused: bool,
    ) -> crate::Result<Command> {
        let current = Scope::current(terminal, actions, focused);
        if self.consumed
            || action.revision != Some(self.revision)
            || self.scope.as_ref() != Some(&current)
            || !current.enabled
        {
            return Err(crate::Error::NativeMenu("stale menu or inactive scope"));
        }
        let command = actions::catalog(terminal)
            .into_iter()
            .find(|entry| entry.info.name == action.name && entry.available)
            .map(|entry| entry.info.command)
            .ok_or(crate::Error::NativeMenu("command unavailable"))?;
        // Consume before dispatch, including local actions and failed requests.
        self.consumed = true;
        Ok(command)
    }
}

impl Smoke {
    pub(crate) fn run_menu_shortcut(&mut self, command: Command) -> crate::Result<()> {
        // No UIKit menu exists before the first installation (also headless tests).
        if self.native_menus.scope.is_none() {
            return self.run_action(command);
        }
        let name = actions::catalog(&self.terminal)
            .into_iter()
            .find(|a| a.info.command == command)
            .ok_or(crate::Error::NativeMenu("unsupported shortcut"))?
            .info
            .name;
        let action = NativeCommand {
            name: name.into(),
            revision: Some(self.native_menus.revision),
        };
        let command = self
            .native_menus
            .take(&action, &self.terminal, &self.actions, true)?;
        self.run_action(command)
    }

    pub(crate) fn install_native_menus(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.native_menus.bindings_installed {
            let bindings = actions::catalog(&self.terminal)
                .into_iter()
                .flat_map(|entry| {
                    entry.shortcuts.iter().map(move |shortcut| {
                        KeyBinding::new(
                            shortcut,
                            NativeCommand {
                                name: entry.info.name.into(),
                                revision: None,
                            },
                            Some("P12MenuDisplay"),
                        )
                    })
                });
            // This context is never attached to a view. It supplies UIKit's
            // shortcut metadata without a second GPUI keyboard dispatch path.
            cx.bind_keys(bindings);
            self.native_menus.bindings_installed = true;
        }
        let focused = self.terminal_focus.is_focused(window) && window.is_window_active();
        if !self
            .native_menus
            .refresh(&self.terminal, &self.actions, focused)
        {
            return;
        }
        let revision = self.native_menus.revision;
        let items = self.native_menus.rows.iter().map(|(name, label, enabled)| {
            MenuItem::action(
                label.clone(),
                NativeCommand {
                    name: name.clone(),
                    revision: Some(revision),
                },
            )
            .disabled(!enabled)
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
        let result = self
            .native_menus
            .take(action, &self.terminal, &self.actions, focused)
            .and_then(|command| self.run_action(command));
        self.observation.record(serde_json::json!({
            "kind": "native-menu", "name": action.name, "revision": action.revision,
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

#[cfg(test)]
#[path = "native_menus_adapter_tests.rs"]
mod adapter_tests;
