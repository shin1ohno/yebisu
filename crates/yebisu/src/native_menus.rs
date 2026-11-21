//! Keyless UIKit commands: labels advertise shortcuts without registering keys.
//! Native stale tags are discarded by the pinned SDK before action dispatch.
use crate::{
    actions,
    app::Smoke,
    board::snapshot,
    shell::{
        ChromeState, HerdrKind,
        edit_bar::COPY_ONE_LINE_LABEL,
        host_menus::{self, MenuItemId, MenuScope},
        sidebar::AgentIdentity,
    },
    terminal_view::TerminalView,
};
use gpui::{Context, Menu, MenuItem, Window};
use sheprd_commands::Command;

const COPY_ONE_LINE: &str = "copy-as-one-line";
const PIN_AGENT: &str = "pin-agent";
const UNPIN_AGENT: &str = "unpin-agent";

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
impl NativeCommand {
    /// The menu item a `NativeCommand` stands for, by catalog or item name.
    /// Tests only: the app builds its commands in `install_native_menus`.
    #[cfg(test)]
    pub(crate) fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Shared Settings/menu presentation API. No platform keymap is modified.
/// Search opens from its shortcut and the palette; the keyless OS menu keeps
/// its pre-search command set.
pub(crate) fn in_os_menu(row: &actions::Action) -> bool {
    row.info.command != Command::Find
}
/// The Settings sheet supplies its intended terminal focus/scene context.
pub(crate) fn catalog_rows(
    terminal: &TerminalView,
    actions: &actions::Actions,
    focused: bool,
) -> Vec<actions::Action> {
    actions::catalog(terminal)
        .into_iter()
        .filter(in_os_menu)
        .map(|mut row| {
            row.available &=
                focused && !(actions.terminal_priority && terminal.input.options.command_control);
            row
        })
        .collect()
}

/// Menu glyphs for catalog shortcuts ("cmd-d" -> "⌘D"), joined by " / ".
/// Shared by the OS menu and the Shortcuts popover.
pub(crate) fn shortcut_glyphs(shortcuts: &[&str]) -> String {
    shortcuts
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
        .join(" / ")
}

fn menu_label(row: &actions::Action) -> String {
    let shortcuts = shortcut_glyphs(row.shortcuts);
    if shortcuts.is_empty() {
        row.label.clone()
    } else {
        format!("{}　{shortcuts}", row.label)
    }
}

/// One OS menu row: (`NativeCommand` name, label, enabled).
type MenuRow = (String, String, bool);
/// The Host and Space menus: title and rows.
type HostMenuRows = Vec<(&'static str, Vec<MenuRow>)>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentPin {
    id: AgentIdentity,
    boot: String,
    generation: u64,
    selection: u64,
    pinned: bool,
}

impl AgentPin {
    fn name(&self) -> &'static str {
        if self.pinned { UNPIN_AGENT } else { PIN_AGENT }
    }

    fn row(&self) -> MenuRow {
        let title = if self.pinned {
            "Unpin Agent"
        } else {
            "Pin Agent"
        };
        (self.name().into(), title.into(), true)
    }
}

fn copy_as_one_line_row(enabled: bool) -> MenuRow {
    (
        COPY_ONE_LINE.into(),
        format!("{COPY_ONE_LINE_LABEL}　⌥⌘C"),
        enabled,
    )
}

fn menu_item((name, label, enabled): &MenuRow) -> MenuItem {
    MenuItem::action(label.clone(), NativeCommand { name: name.clone() }).disabled(!enabled)
}

fn edit_menu(enabled: bool) -> Menu {
    Menu::new("Edit").items([menu_item(&copy_as_one_line_row(enabled))])
}

#[derive(Default)]
pub(crate) struct NativeMenus {
    scope: Option<Scope>,
    rows: Vec<MenuRow>,
    copy_as_one_line: bool,
    agent_pin: Option<AgentPin>,
    /// The Host and Space menus (L6, Q8): title and (name, label, enabled).
    pub(crate) host_menus: HostMenuRows,
    /// The target the Host and Space menus were built for (S-10).
    menu_scope: MenuScope,
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

    /// The Host and Space menus (L6): rebuilt when their target or rows change.
    fn refresh_host_menus(
        &mut self,
        scope: MenuScope,
        space_name: &str,
        state: &ChromeState,
        focused: bool,
    ) -> bool {
        let host_menus = host_menu_rows(&scope, space_name, state, focused);
        if self.host_menus == host_menus && self.menu_scope == scope {
            return false;
        }
        self.host_menus = host_menus;
        self.menu_scope = scope;
        true
    }

    /// The target the Host and Space menus were last built for (tests only).
    #[cfg(test)]
    pub(crate) fn menu_scope(&self) -> &MenuScope {
        &self.menu_scope
    }

    /// QA only: the Scope and rows the OS menu is being rebuilt from, for the
    /// `p12-menu-build` record. Everything `refresh` compares is included, so a
    /// rebuild caused by the Scope alone (host, pane, boot, epoch) is visible.
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    pub(crate) fn p12_build_record(&self) -> serde_json::Value {
        let scope = self.scope.as_ref();
        serde_json::json!({
            "host": scope.map(|s| s.host.as_str()),
            "generation": scope.map(|s| s.generation),
            "boot": scope.map(|s| s.boot.as_str()),
            "workspace": scope.and_then(|s| s.workspace.as_deref()),
            "tab": scope.and_then(|s| s.tab.as_deref()),
            "pane": scope.and_then(|s| s.pane.as_deref()),
            "epoch": scope.map(|s| s.epoch),
            "methods": scope.map(|s| s.methods.len()),
            "palette": scope.map(|s| s.palette),
            "focused": scope.map(|s| s.focused),
            "availability": self.rows.iter().map(|(_, _, enabled)| *enabled).collect::<Vec<_>>(),
        })
    }

    fn command_gate(
        &self,
        command: Command,
        from_menu: bool,
        terminal: &TerminalView,
        actions: &actions::Actions,
        focused: bool,
    ) -> crate::Result<()> {
        if !focused {
            return Err(crate::Error::NativeMenu("inactive terminal focus"));
        }
        // Approach C: only a menu selection carries displayed-scope authority.
        // A physical key is a new intent and uses the current catalog state.
        if from_menu && self.scope.as_ref() != Some(&Scope::current(terminal, actions, focused)) {
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
        let terminal_menu = self
            .native_menus
            .refresh(&self.terminal, &self.actions, focused);
        let (scope, state) = (self.current_menu_scope(), self.menu_chrome_state());
        let space_name = self.menu_space_name().unwrap_or_default();
        let host_menus = self
            .native_menus
            .refresh_host_menus(scope, &space_name, &state, focused);
        let copy_as_one_line = self.native_copy_as_one_line_available(focused);
        let agent_pin = self.native_agent_pin(focused);
        let extras_changed = self.native_menus.copy_as_one_line != copy_as_one_line
            || self.native_menus.agent_pin != agent_pin;
        self.native_menus.copy_as_one_line = copy_as_one_line;
        self.native_menus.agent_pin = agent_pin;
        if !terminal_menu && !host_menus && !extras_changed {
            return;
        }
        // QA only: `refresh` returned true, so the OS menu is rebuilt below.
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.p12_note_menu_rebuild();
        let pin = self
            .native_menus
            .agent_pin
            .as_ref()
            .map(|pin| menu_item(&pin.row()));
        let items = self.native_menus.rows.iter().map(menu_item).chain(pin);
        // The first GPUI menu joins the application menu. UIKit matches
        // the following "Edit" title to UIMenuEdit in the pinned SDK.
        let mut menus = vec![
            Menu::new("Terminal").items(items),
            edit_menu(copy_as_one_line),
        ];
        menus.extend(
            self.native_menus
                .host_menus
                .iter()
                .map(|(title, rows)| Menu::new(*title).items(rows.iter().map(menu_item))),
        );
        cx.set_menus(menus);
    }

    fn native_copy_as_one_line_available(&self, focused: bool) -> bool {
        focused
            && !self.input_modal()
            && !self.new_space_open()
            && !(self.actions.terminal_priority && self.terminal.input.options.command_control)
            && self.handoff.selection.is_some()
    }

    fn native_agent_pin(&self, focused: bool) -> Option<AgentPin> {
        if !focused
            || self.input_modal()
            || self.new_space_open()
            || self.terminal.core_mode
            || !self.terminal.live.connected
        {
            return None;
        }
        let shown = snapshot(&self.terminal.live)?;
        let pane = shown.focused_pane_id.as_ref()?;
        if !shown.agents.iter().any(|agent| agent.pane_id == *pane) {
            return None;
        }
        let id = AgentIdentity {
            host: self.terminal.reconnect.target.clone(),
            kind: HerdrKind::Herdr1,
            pane: pane.clone(),
        };
        Some(AgentPin {
            pinned: self.shell.sidebar.is_agent_pinned(&id, &shown.boot_id),
            id,
            boot: shown.boot_id.clone(),
            generation: self.terminal.link.generation,
            selection: self.board.selection,
        })
    }

    fn run_native_edit(&mut self, focused: bool, cx: &mut Context<Self>) -> crate::Result<()> {
        if !self.native_copy_as_one_line_available(focused)
            || !self.native_menus.copy_as_one_line
            || self.native_menus.scope.as_ref()
                != Some(&Scope::current(&self.terminal, &self.actions, focused))
        {
            return Err(crate::Error::NativeMenu("Copy as One Line unavailable"));
        }
        self.copy_selection_as_one_line(cx);
        Ok(())
    }

    fn run_native_pin(
        &mut self,
        name: &str,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let target = self
            .native_agent_pin(focused)
            .filter(|pin| pin.name() == name && Some(pin) == self.native_menus.agent_pin.as_ref())
            .ok_or(crate::Error::NativeMenu(
                "Agent pane changed; reopen the menu",
            ))?;
        self.toggle_sidebar_agent_pin(target.id, &target.boot, target.generation, cx)
    }

    pub(crate) fn native_command_action(
        &mut self,
        action: &NativeCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused = self.terminal_focus.is_focused(window) && window.is_window_active();
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let receipt = self.p12_menu_begin(&action.name, "native_action", window);
        let result = if action.name == COPY_ONE_LINE {
            self.run_native_edit(focused, cx)
        } else if matches!(action.name.as_str(), PIN_AGENT | UNPIN_AGENT) {
            self.run_native_pin(&action.name, focused, cx)
        } else if let Some(item) = MenuItemId::from_name(&action.name) {
            // Host / Space (L6): the target shown with the menu must still be
            // the current one (S-10).
            let shown = self.native_menus.menu_scope.clone();
            if focused {
                self.run_menu_item(item, &shown, window, cx)
            } else {
                Err(crate::Error::NativeMenu("inactive terminal focus"))
            }
        } else {
            actions::catalog(&self.terminal)
                .iter()
                .find(|row| row.info.name == action.name)
                .map(|row| row.info.command)
                .ok_or(crate::Error::NativeMenu("unsupported command"))
                .and_then(|command| self.run_catalog_command(command, Some(action), focused))
        };
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

/// The Host and Space menus' rows for the menu bar. Items name their target
/// ("Reconnect — studio"); every item is disabled without terminal focus.
/// The Host and Space menus' rows; Space items name the space as the
/// sidebar does (`space_name`), not by its workspace id.
fn host_menu_rows(
    scope: &MenuScope,
    space_name: &str,
    state: &ChromeState,
    focused: bool,
) -> HostMenuRows {
    host_menus::menu_bar(state, scope.space.is_some())
        .into_iter()
        .map(|(title, entries)| {
            let target = if title == host_menus::HOST_MENU {
                scope.host.as_str()
            } else {
                space_name
            };
            let rows = entries
                .into_iter()
                .map(|entry| {
                    let label = host_menus::menu_item_label(entry.item, target);
                    (
                        entry.item.name().to_owned(),
                        label,
                        entry.enabled && focused,
                    )
                })
                .collect();
            (title, rows)
        })
        .collect()
}

#[cfg(test)]
#[path = "native_menus_tests.rs"]
mod tests;
