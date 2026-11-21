//! Sidebar footer and the New space control in its header (Main.dc.html).
use super::{
    ChromeState, EntryAction, EntryPoint, HerdrKind, Place,
    sidebar::icons::Icon,
    style::{color, metric, radius},
};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Role, Window, div, prelude::*, px, rgb};
use sheprd_commands::Command;

pub(crate) mod keys;

pub(crate) const NEW_SPACE: &str = "New space";
pub(crate) const HOSTS: &str = "Hosts";
pub(crate) const SETTINGS: &str = "Settings";
/// One footer row (Main.dc.html: buttons 44 high).
pub(crate) const FOOTER_ROW_HEIGHT: f32 = metric::TOUCH;
/// Padding above and below the footer row (Main.dc.html `padding: 4px 8px`).
pub(crate) const FOOTER_PADDING: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterItem {
    NewSpace,
    Hosts,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterEntry {
    pub(crate) item: FooterItem,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

impl FooterItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NewSpace => NEW_SPACE,
            Self::Hosts => HOSTS,
            Self::Settings => SETTINGS,
        }
    }

    const fn action(self) -> EntryAction {
        match self {
            Self::NewSpace => EntryAction::NewSpace,
            Self::Hosts => EntryAction::Hosts,
            Self::Settings => EntryAction::Settings,
        }
    }

    const fn element_id(self) -> &'static str {
        match self {
            Self::NewSpace => "shell-footer-new-space",
            Self::Hosts => "shell-footer-hosts",
            Self::Settings => "shell-footer-settings",
        }
    }
}

/// The footer, top to bottom and left to right.
pub(crate) fn footer_items(_kind: HerdrKind) -> Vec<FooterEntry> {
    vec![
        FooterEntry {
            item: FooterItem::Hosts,
            enabled: true,
            reason: None,
        },
        FooterEntry {
            item: FooterItem::Settings,
            enabled: true,
            reason: None,
        },
    ]
}

/// The controls in the expanded sidebar chrome.
pub(crate) fn footer_entry_points(state: &ChromeState) -> Vec<EntryPoint> {
    if state.sidebar_hidden {
        return Vec::new();
    }
    footer_items(state.kind)
        .into_iter()
        .map(|entry| EntryPoint {
            action: entry.item.action(),
            place: Place::SidebarFooter,
            enabled: entry.enabled,
            reason: entry.reason,
        })
        .chain(std::iter::once(EntryPoint {
            action: EntryAction::NewSpace,
            place: Place::SidebarHeader,
            enabled: true,
            reason: None,
        }))
        .collect()
}

/// The legacy rail action requests a Herdr 1 workspace.
pub(crate) fn new_space_command(kind: HerdrKind) -> Option<Command> {
    (kind == HerdrKind::Herdr1).then_some(Command::Workspace)
}

/// Where footer Hosts goes: the L7 sheet, or the Keys and hosts screen when
/// `open_hosts` declines (plan v2 5.2: "FT (SP へ戻る)" until L7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostsTarget {
    Sheet,
    KeysAndHosts,
}

pub(crate) const fn hosts_target(sheet_opened: bool) -> HostsTarget {
    if sheet_opened {
        HostsTarget::Sheet
    } else {
        HostsTarget::KeysAndHosts
    }
}

impl Smoke {
    /// Footer controls and the legacy rail action.
    pub(crate) fn footer_tap(
        &mut self,
        item: FooterItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = self.chrome_state().kind;
        match item {
            FooterItem::NewSpace => {
                if let Some(command) = new_space_command(kind)
                    && let Err(error) = self.run_action(command)
                {
                    self.terminal.live.status = error.to_string();
                }
            }
            FooterItem::Hosts => match hosts_target(self.open_hosts(cx)) {
                HostsTarget::Sheet => self.hand_keys_to_sheet(window, cx),
                HostsTarget::KeysAndHosts => self.return_to_keys_and_hosts(window, cx),
            },
            FooterItem::Settings => self.begin_settings_modal(window, cx),
        }
        cx.notify();
    }

    /// The old toolbar Hosts button: close the terminal and show the Keys
    /// and hosts screen (the L7 first-run frame since L7).
    pub(crate) fn return_to_keys_and_hosts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.blur(cx);
        self.terminal.input.detach();
        self.handoff.cancel();
        self.board.selection = self.board.selection.wrapping_add(1);
        self.terminal.open = false;
        self.smoke_mode = false;
    }

    pub(crate) fn render_sidebar_footer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        Some(
            div()
                .id("shell-sidebar-footer")
                .debug_selector(|| "shell-sidebar-footer".into())
                .flex_none()
                .flex()
                .items_center()
                .w_full()
                .border_t_1()
                .border_color(rgb(color::RAISED))
                .px(px(metric::INLINE_GAP))
                .py(px(FOOTER_PADDING))
                .children(
                    footer_items(self.chrome_state().kind)
                        .into_iter()
                        .map(|entry| footer_button(entry.item, cx)),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn render_new_space_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let item = FooterItem::NewSpace;
        Icon::Add
            .button(item.element_id(), item.label(), true)
            .on_click(cx.listener(|this, _, _, cx| this.open_new_space(cx)))
            .into_any_element()
    }
}

fn footer_button(item: FooterItem, cx: &mut Context<Smoke>) -> AnyElement {
    let id = item.element_id();
    let button = match item {
        FooterItem::Hosts => div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .role(Role::Button)
            .aria_label(HOSTS)
            .flex()
            .flex_1()
            .items_center()
            .gap(px(metric::ROW_GAP))
            .px(px(metric::INLINE_GAP))
            .rounded(px(radius::CONTROL))
            .h(px(FOOTER_ROW_HEIGHT))
            .child(Icon::Hosts.draw(color::MUTED))
            .child(HOSTS),
        FooterItem::Settings => {
            Icon::Settings
                .button(id, SETTINGS, true)
                .children(crate::qa_ax::mark(
                    crate::qa_ax::ids::FOOTER_SETTINGS,
                    SETTINGS,
                ))
        }
        FooterItem::NewSpace => Icon::Add.button(id, NEW_SPACE, true),
    };
    button
        .on_click(cx.listener(move |this, _, window, cx| this.footer_tap(item, window, cx)))
        .into_any_element()
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
