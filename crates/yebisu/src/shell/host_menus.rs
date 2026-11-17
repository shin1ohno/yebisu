//! L6: Host / Space menus in the menu bar and on long press (Q8). Owner: L6.
//!
//! The menu bar (`native_menus.rs`) and the long-press menus of the sidebar's
//! host and space rows list the same items, built here from one item table
//! per menu. A menu remembers the target it was shown for ([`MenuScope`]);
//! a pick whose target has changed since is refused (S-10), with the same
//! strength as the displayed-scope check of the Terminal menu
//! (`NativeMenus::command_gate`).
use super::{ChromeState, HerdrKind};
use crate::{app::Smoke, board::snapshot};
use gpui::{Context, Window};

pub(crate) const HOST_MENU: &str = "Host";
pub(crate) const SPACE_MENU: &str = "Space";
const ALREADY_CONNECTED: &str = "Already connected";
const NOT_CONNECTED: &str = "Not connected";
const HERDR1_NO_DETACH: &str = "Herdr 1 sessions stay attached";
const HERDR2_NO_NEW_TAB: &str = "New tab is not available on Herdr 2";
const NO_SPACE: &str = "No space selected";
/// Host and space names longer than this end in "…" in menu items.
pub(crate) const MENU_NAME_MAX: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostMenuItem {
    Reconnect,
    Detach,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpaceMenuItem {
    NewTab,
    Detach,
}

/// Menu bar order of the Host menu.
const HOST_MENU_ITEMS: [HostMenuItem; 3] = [
    HostMenuItem::Reconnect,
    HostMenuItem::Detach,
    HostMenuItem::Edit,
];
/// Long press of a host row (Q8: the same items as the menu bar).
const HOST_LONG_PRESS: [HostMenuItem; 3] = [
    HostMenuItem::Reconnect,
    HostMenuItem::Detach,
    HostMenuItem::Edit,
];
/// Menu bar order of the Space menu.
const SPACE_MENU_ITEMS: [SpaceMenuItem; 2] = [SpaceMenuItem::NewTab, SpaceMenuItem::Detach];
/// Long press of a space row (Q8).
const SPACE_LONG_PRESS: [SpaceMenuItem; 2] = [SpaceMenuItem::NewTab, SpaceMenuItem::Detach];

impl HostMenuItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Reconnect => "Reconnect",
            Self::Detach => "Detach",
            Self::Edit => "Edit",
        }
    }
    /// The `NativeCommand` name of the menu bar item.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Reconnect => "host.reconnect",
            Self::Detach => "host.detach",
            Self::Edit => "host.edit",
        }
    }
}

impl SpaceMenuItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NewTab => "New tab",
            Self::Detach => "Detach",
        }
    }
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::NewTab => "space.new_tab",
            Self::Detach => "space.detach",
        }
    }
}

/// One Host or Space item, wherever it is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuItemId {
    Host(HostMenuItem),
    Space(SpaceMenuItem),
}

impl MenuItemId {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Host(item) => item.label(),
            Self::Space(item) => item.label(),
        }
    }
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Host(item) => item.name(),
            Self::Space(item) => item.name(),
        }
    }
    /// The item a `NativeCommand` name stands for; `None` for catalog names.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        HOST_MENU_ITEMS
            .into_iter()
            .map(Self::Host)
            .chain(SPACE_MENU_ITEMS.into_iter().map(Self::Space))
            .find(|item| item.name() == name)
    }
}

/// A listed item: enabled, or disabled with its reason (q4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MenuEntry {
    pub(crate) item: MenuItemId,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

fn entry(item: MenuItemId, enabled: bool, reason: &'static str) -> MenuEntry {
    MenuEntry {
        item,
        enabled,
        reason: (!enabled).then_some(reason),
    }
}

fn host_entry(item: HostMenuItem, state: &ChromeState) -> MenuEntry {
    let id = MenuItemId::Host(item);
    match item {
        HostMenuItem::Reconnect => entry(id, !state.connected, ALREADY_CONNECTED),
        HostMenuItem::Detach => match state.kind {
            HerdrKind::Herdr1 => entry(id, false, HERDR1_NO_DETACH),
            HerdrKind::Herdr2 => entry(id, state.connected, NOT_CONNECTED),
        },
        HostMenuItem::Edit => entry(id, true, NOT_CONNECTED),
    }
}

fn space_entry(item: SpaceMenuItem, state: &ChromeState, has_space: bool) -> MenuEntry {
    let id = MenuItemId::Space(item);
    if !has_space {
        return entry(id, false, NO_SPACE);
    }
    match (item, state.kind) {
        (SpaceMenuItem::NewTab, HerdrKind::Herdr1) => entry(id, state.connected, NOT_CONNECTED),
        (SpaceMenuItem::NewTab, HerdrKind::Herdr2) => entry(id, false, HERDR2_NO_NEW_TAB),
        (SpaceMenuItem::Detach, HerdrKind::Herdr1) => entry(id, false, HERDR1_NO_DETACH),
        (SpaceMenuItem::Detach, HerdrKind::Herdr2) => entry(id, state.connected, NOT_CONNECTED),
    }
}

/// The menu bar's Host and Space menus, in order (Q8).
pub(crate) fn menu_bar(
    state: &ChromeState,
    has_space: bool,
) -> Vec<(&'static str, Vec<MenuEntry>)> {
    vec![
        (
            HOST_MENU,
            HOST_MENU_ITEMS
                .into_iter()
                .map(|item| host_entry(item, state))
                .collect(),
        ),
        (
            SPACE_MENU,
            SPACE_MENU_ITEMS
                .into_iter()
                .map(|item| space_entry(item, state, has_space))
                .collect(),
        ),
    ]
}

/// The long-press menu of a host row whose host is in `state`.
pub(crate) fn host_long_press(state: &ChromeState) -> Vec<MenuEntry> {
    HOST_LONG_PRESS
        .into_iter()
        .map(|item| host_entry(item, state))
        .collect()
}

/// The long-press menu of a space row (a row always names its space).
pub(crate) fn space_long_press(state: &ChromeState) -> Vec<MenuEntry> {
    SPACE_LONG_PRESS
        .into_iter()
        .map(|item| space_entry(item, state, true))
        .collect()
}

/// `name` cut to [`MENU_NAME_MAX`] characters, ending in "…" when cut.
pub(crate) fn menu_name(name: &str) -> String {
    if name.chars().count() <= MENU_NAME_MAX {
        return name.to_owned();
    }
    let mut cut: String = name.chars().take(MENU_NAME_MAX - 1).collect();
    cut.push('…');
    cut
}

/// The menu bar label of an item: its name and its target, e.g.
/// "Reconnect — studio".
pub(crate) fn menu_item_label(item: MenuItemId, target: &str) -> String {
    if target.is_empty() {
        item.label().to_owned()
    } else {
        format!("{} — {}", item.label(), menu_name(target))
    }
}

/// The target captured when a menu was shown (S-10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MenuScope {
    pub(crate) host: String,
    pub(crate) space: Option<String>,
}

/// Whether an item shown for `shown` may still run against `current`.
/// A Host item is shown with `space: None` and needs the same host; a Space
/// item also needs the same space. An empty host never runs.
pub(crate) fn scope_still_valid(shown: &MenuScope, current: &MenuScope) -> bool {
    !shown.host.is_empty()
        && shown.host == current.host
        && (shown.space.is_none() || shown.space == current.space)
}

/// The host an Edit pick opens: the host the menu was shown for, while it
/// is still the selected host.
pub(crate) fn edit_target(shown: &MenuScope, current: &MenuScope) -> Option<String> {
    let host = MenuScope {
        host: shown.host.clone(),
        space: None,
    };
    scope_still_valid(&host, current).then(|| shown.host.clone())
}

impl Smoke {
    /// The selected host and its focused space, as the menus see them now.
    pub(crate) fn current_menu_scope(&self) -> MenuScope {
        MenuScope {
            host: self.terminal.reconnect.target.clone(),
            space: snapshot(&self.terminal.live).and_then(|s| s.focused_workspace_id.clone()),
        }
    }

    /// The focused space's name for the Space menu's labels, as the
    /// sidebar writes it: "Space N · label", or "Space N" without a label.
    pub(crate) fn menu_space_name(&self) -> Option<String> {
        let shown = snapshot(&self.terminal.live)?;
        let id = shown.focused_workspace_id.as_ref()?;
        let space = shown.workspaces.iter().find(|w| &w.workspace_id == id)?;
        Some(if space.label.is_empty() {
            format!("Space {}", space.number)
        } else {
            format!("Space {} · {}", space.number, space.label)
        })
    }

    /// The chrome state the Host and Space menus are built from.
    pub(crate) fn menu_chrome_state(&self) -> ChromeState {
        ChromeState {
            kind: if self.terminal.core_mode {
                HerdrKind::Herdr2
            } else {
                HerdrKind::Herdr1
            },
            connected: self.terminal.live.connected,
            sheet_open: self.settings_modal || self.hosts_sheet_modal(),
            ..ChromeState::default()
        }
    }

    /// Run a Host or Space item shown for `shown`. Refuses a changed target
    /// (S-10) and a disabled item; the caller reports the error.
    pub(crate) fn run_menu_item(
        &mut self,
        item: MenuItemId,
        shown: &MenuScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let current = self.current_menu_scope();
        let target = match item {
            MenuItemId::Host(_) => MenuScope {
                host: shown.host.clone(),
                space: None,
            },
            MenuItemId::Space(_) => shown.clone(),
        };
        if shown.space.is_none() && matches!(item, MenuItemId::Space(_)) {
            return Err(crate::Error::NativeMenu("no space was shown"));
        }
        if !scope_still_valid(&target, &current) {
            return Err(crate::Error::NativeMenu("menu target changed"));
        }
        let state = self.menu_chrome_state();
        let enabled = match item {
            MenuItemId::Host(item) => host_entry(item, &state).enabled,
            MenuItemId::Space(item) => space_entry(item, &state, current.space.is_some()).enabled,
        };
        if !enabled {
            return Err(crate::Error::NativeMenu("menu item unavailable"));
        }
        match item {
            MenuItemId::Host(HostMenuItem::Reconnect) => {
                if self.terminal.live.version_mismatch.get().is_some() {
                    return Err(crate::Error::NativeMenu("version mismatch"));
                }
                self.connect_terminal(cx);
            }
            MenuItemId::Host(HostMenuItem::Detach) | MenuItemId::Space(SpaceMenuItem::Detach) => {
                self.detach_core_and_blur(window, cx);
            }
            MenuItemId::Host(HostMenuItem::Edit) => {
                let host = edit_target(shown, &current)
                    .ok_or(crate::Error::NativeMenu("menu target changed"))?;
                self.shell.shortcuts.edit_host = Some(host);
                if !self.open_hosts(cx) {
                    // Without the Hosts sheet, Settings holds the hosts page.
                    self.begin_settings_modal(window, cx);
                }
            }
            MenuItemId::Space(SpaceMenuItem::NewTab) => {
                self.run_action(sheprd_commands::Command::Tab)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "host_menus_tests.rs"]
mod tests;
