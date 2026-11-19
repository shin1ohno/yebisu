//! L2b: the sidebar footer (New space / Hosts / Settings). Owner: L2b.
//!
//! Plan v2 7.6: under the sidebar list, New space on a 44 pt row, then Hosts
//! and Settings on a second 44 pt row (Main.dc.html; L0
//! `SIDEBAR_FOOTER_HEIGHT` = rule 1 + padding 4 + 4 + 44 + 44). New space
//! sends `Command::Workspace` on Herdr 1 and is disabled with its reason on
//! Herdr 2 (q4). Hosts opens the L7 Hosts sheet and hands the hardware keys
//! to it (`footer/keys.rs`, review cf8bbcccc2164abe F1); when `open_hosts`
//! declines, it returns to the Keys and hosts screen as the toolbar button
//! did. Settings opens the L1 settings sheet.
use super::{ChromeState, EntryAction, EntryPoint, HerdrKind, Place};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};
use sheprd_commands::Command;

pub(crate) mod keys;

pub(crate) const NEW_SPACE: &str = "New space";
pub(crate) const HOSTS: &str = "Hosts";
pub(crate) const SETTINGS: &str = "Settings";
/// q4: Herdr 2 sessions are a separate task; New space says so.
pub(crate) const HERDR2_NEW_SPACE: &str = "Herdr 2 sessions come in a later update";
/// One footer row (Main.dc.html: buttons 44 high).
pub(crate) const FOOTER_ROW_HEIGHT: f32 = 44.0;
/// Padding above and below the two rows (Main.dc.html `padding: 4px 8px`).
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
pub(crate) fn footer_items(kind: HerdrKind) -> Vec<FooterEntry> {
    let herdr1 = kind == HerdrKind::Herdr1;
    vec![
        FooterEntry {
            item: FooterItem::NewSpace,
            enabled: herdr1,
            reason: (!herdr1).then_some(HERDR2_NEW_SPACE),
        },
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

/// The footer's places; none while the user hid the sidebar (the frame then
/// draws no sidebar column; "Show sidebar ⌘B" in the status bar brings it back).
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
        .collect()
}

/// The command New space sends: a new Herdr 1 workspace; none on Herdr 2.
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
    /// One footer tap. A disabled entry does nothing.
    pub(crate) fn footer_tap(
        &mut self,
        item: FooterItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = self.chrome_state().kind;
        let enabled = footer_items(kind)
            .iter()
            .any(|entry| entry.item == item && entry.enabled);
        if !enabled {
            return;
        }
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
        let mut new_space = None;
        let mut row = div().flex().flex_none().w_full().h(px(FOOTER_ROW_HEIGHT));
        for entry in footer_items(self.chrome_state().kind) {
            let button = footer_button(&entry, cx);
            if entry.item == FooterItem::NewSpace {
                new_space = Some(button);
            } else {
                row = row.child(button);
            }
        }
        Some(
            div()
                .id("shell-sidebar-footer")
                .flex_none()
                .flex()
                .flex_col()
                .w_full()
                .border_t_1()
                .border_color(rgb(0x2b2933))
                .px_2()
                .py(px(FOOTER_PADDING))
                .children(new_space)
                .child(row)
                .into_any_element(),
        )
    }
}

fn footer_button(entry: &FooterEntry, cx: &mut Context<Smoke>) -> AnyElement {
    let item = entry.item;
    let id = item.element_id();
    let button = div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .px_2()
        .overflow_hidden()
        .whitespace_nowrap()
        .rounded(px(8.))
        .h(px(FOOTER_ROW_HEIGHT))
        .text_color(rgb(if entry.enabled { 0xd8dee9 } else { 0x6b7280 }))
        .child(item.label())
        .children(
            (item == FooterItem::Settings)
                .then(|| crate::qa_ax::mark(crate::qa_ax::ids::FOOTER_SETTINGS, SETTINGS))
                .flatten(),
        )
        .children(entry.reason.map(|reason| {
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_xs()
                .text_color(rgb(0x8a8f98))
                .child(reason)
        }));
    let button = if item == FooterItem::NewSpace {
        button.w_full()
    } else {
        button.flex_1()
    };
    if entry.enabled {
        button
            .on_click(cx.listener(move |this, _, window, cx| this.footer_tap(item, window, cx)))
            .into_any_element()
    } else {
        button.into_any_element()
    }
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
