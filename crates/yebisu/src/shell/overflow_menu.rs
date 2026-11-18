//! L2a: the tab row's ... menu. Owner: L2a.
//!
//! Herdr 1: Paste, Paste image, Select text, Copy (while selecting), Find,
//! Command palette, Reconnect, Close tab. Herdr 2: Paste, Paste image (q2),
//! Select text, Copy, Find (when PR 26's `available` is true), Reconnect,
//! Detach, New tab (disabled with its reason, q4). Plan v2 7.5.
use super::{ChromeState, HerdrKind, herdr2_frame::panes};
use crate::app::Smoke;
use gpui::{AnyElement, Context, MouseButton, Window, div, prelude::*, px, rgb};
use sheprd_commands::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverflowItem {
    Paste,
    PasteImage,
    SelectText,
    Copy,
    Find,
    CommandPalette,
    Reconnect,
    CloseTab,
    Detach,
    NewTab,
    /// Herdr 2's band controls (L8): the band retires once these are here.
    Split,
    Pane1,
    Pane2,
    SplitDirection,
}

impl OverflowItem {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Paste => "Paste",
            Self::PasteImage => "Paste image",
            Self::SelectText => "Select text",
            Self::Copy => "Copy",
            Self::Find => "Find",
            Self::CommandPalette => "Command palette",
            Self::Reconnect => "Reconnect",
            Self::CloseTab => "Close tab",
            Self::Detach => panes::DETACH,
            Self::NewTab => "New tab",
            Self::Split => panes::SPLIT,
            Self::Pane1 => panes::PANE_1,
            Self::Pane2 => panes::PANE_2,
            Self::SplitDirection => panes::ORIENTATION,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OverflowEntry {
    pub(crate) item: OverflowItem,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

pub(crate) const NOT_CONNECTED: &str = "Not connected";
pub(crate) const ALREADY_CONNECTED: &str = "Already connected";
pub(crate) const FIND_UNAVAILABLE: &str = "Find is not available for this terminal";
/// q4: one Herdr 2 session is one tab until multi-session work lands.
pub(crate) const HERDR2_NEW_TAB: &str = "Herdr 2 shows one tab per session for now";

fn entry(item: OverflowItem, enabled: bool, reason: &'static str) -> OverflowEntry {
    OverflowEntry {
        item,
        enabled,
        reason: (!enabled).then_some(reason),
    }
}

/// The ... menu for Herdr 1 or Herdr 2 (plan v2 7.5).
pub(crate) fn overflow_menu_items(state: &ChromeState) -> Vec<OverflowEntry> {
    let online = state.connected;
    let mut items = vec![
        entry(OverflowItem::Paste, online, NOT_CONNECTED),
        // q2: Herdr 2 supports image paste since PR 37.
        entry(OverflowItem::PasteImage, online, NOT_CONNECTED),
        entry(OverflowItem::SelectText, online, NOT_CONNECTED),
    ];
    match state.kind {
        HerdrKind::Herdr1 => {
            if state.selecting {
                items.push(entry(OverflowItem::Copy, true, NOT_CONNECTED));
            }
            items.push(entry(
                OverflowItem::Find,
                state.find_available,
                FIND_UNAVAILABLE,
            ));
            items.push(entry(OverflowItem::CommandPalette, true, NOT_CONNECTED));
            items.push(entry(OverflowItem::Reconnect, !online, ALREADY_CONNECTED));
            items.push(entry(OverflowItem::CloseTab, online, NOT_CONNECTED));
        }
        HerdrKind::Herdr2 => {
            items.push(entry(OverflowItem::Copy, online, NOT_CONNECTED));
            if state.find_available {
                items.push(entry(OverflowItem::Find, true, FIND_UNAVAILABLE));
            }
            items.push(entry(OverflowItem::Reconnect, !online, ALREADY_CONNECTED));
            // The control band's controls (L8, plan v2 5.2): same labels.
            for item in [
                OverflowItem::Split,
                OverflowItem::Pane1,
                OverflowItem::Pane2,
                OverflowItem::SplitDirection,
            ] {
                items.push(entry(item, online, NOT_CONNECTED));
            }
            items.push(entry(OverflowItem::Detach, online, NOT_CONNECTED));
            items.push(entry(OverflowItem::NewTab, false, HERDR2_NEW_TAB));
        }
    }
    items
}

impl Smoke {
    /// The open ... menu, anchored under the tab row's right edge; `None`
    /// while closed.
    pub(crate) fn render_overflow_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.shell.frame.overflow_open {
            return None;
        }
        let entries = overflow_menu_items(&self.chrome_state());
        Some(
            div()
                .id("shell-overflow-menu")
                .debug_selector(|| "shell-overflow-menu".into())
                // A press on an item must not also reach the terminal under
                // it (selection_down), as the edit bar and the Hosts sheet
                // already occlude (review SF-2).
                .occlude()
                // A press outside the terminal column (the sidebar) closes it.
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.shell.frame.overflow_open = false;
                    cx.notify();
                }))
                .absolute()
                .top(px(crate::layout::TAB_ROW_HEIGHT))
                .right_0()
                .w(px(240.))
                .flex()
                .flex_col()
                .bg(rgb(0x2b2933))
                .text_color(rgb(0xd8dee9))
                .children(entries.into_iter().enumerate().map(|(index, entry)| {
                    let item = entry.item;
                    div()
                        .id(("shell-overflow-item", index))
                        .h(px(44.))
                        .px_4()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .text_color(rgb(if entry.enabled { 0xd8dee9 } else { 0x777777 }))
                        .child(item.label())
                        .children(entry.reason.map(|reason| div().text_xs().child(reason)))
                        .when(entry.enabled, |view| {
                            view.on_click(cx.listener(move |this, _, window, cx| {
                                this.shell.frame.overflow_open = false;
                                this.run_overflow_item(item, window, cx);
                                cx.notify();
                            }))
                        })
                }))
                .into_any_element(),
        )
    }

    /// While the menu is open, a transparent layer over the terminal column
    /// takes the press that closes it, so that press neither starts a
    /// selection in the terminal nor reopens the menu from the ... button.
    pub(crate) fn render_overflow_backdrop(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.shell.frame.overflow_open {
            return None;
        }
        Some(
            div()
                .id("shell-overflow-backdrop")
                .debug_selector(|| "shell-overflow-backdrop".into())
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.shell.frame.overflow_open = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .into_any_element(),
        )
    }

    fn run_overflow_item(
        &mut self,
        item: OverflowItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = match item {
            OverflowItem::Paste => {
                self.start_paste(false, cx);
                Ok(())
            }
            OverflowItem::PasteImage => {
                self.start_paste(true, cx);
                Ok(())
            }
            OverflowItem::SelectText => {
                self.toggle_selection();
                Ok(())
            }
            OverflowItem::Copy => {
                self.copy_selection(cx);
                Ok(())
            }
            OverflowItem::Find => {
                self.open_find(window, cx);
                Ok(())
            }
            OverflowItem::CommandPalette => self.run_action(Command::Palette),
            OverflowItem::Reconnect => {
                self.reconnect_from_chrome(cx);
                Ok(())
            }
            OverflowItem::CloseTab => self.run_action(Command::CloseTab),
            OverflowItem::Detach => {
                self.detach_core_and_blur(window, cx);
                Ok(())
            }
            OverflowItem::NewTab => self.run_action(Command::Tab),
            OverflowItem::Split => {
                self.core_command(crate::core::Command::Split);
                Ok(())
            }
            OverflowItem::Pane1 => {
                self.core_command(crate::core::Command::Focus(0));
                Ok(())
            }
            OverflowItem::Pane2 => {
                self.core_command(crate::core::Command::Focus(1));
                Ok(())
            }
            OverflowItem::SplitDirection => {
                self.core_command(crate::core::Command::Orientation);
                Ok(())
            }
        };
        if let Err(e) = result {
            self.terminal.live.status = e.to_string();
        }
    }

    /// The toolbar's Reconnect guard, shared by the status bar and the menu.
    pub(crate) fn reconnect_from_chrome(&mut self, cx: &mut Context<Self>) {
        if !self.terminal.live.connected && self.terminal.live.version_mismatch.get().is_none() {
            self.connect_terminal(cx);
        }
    }
}

#[cfg(test)]
#[path = "overflow_menu_tests.rs"]
mod tests;
