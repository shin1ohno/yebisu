//! The tab row's … menu. Herdr 2 puts tab and split commands first, then
//! clipboard and connection actions (Herdr2.dc.html:123–132).
use super::{
    ChromeState, HerdrKind,
    herdr2_frame::panes,
    style::{color, metric, radius, shadow, text},
};
use crate::{actions, app::Smoke};
use gpui::{AnyElement, Context, MouseButton, Window, div, prelude::*, px, rgb};
use sheprd_commands::Command;

// Herdr2.dc.html:123. The menu is inset from the column and clears the tab row.
const MENU_RIGHT: f32 = 8.;
const MENU_TOP: f32 = 40.;
const DIVIDER_COLOR: u32 = 0x3b3945;
const DETACH_LABEL: &str = "Detach (keeps terminals running)";
const COMMAND_UNAVAILABLE: &str = "Action unavailable";
const HERDR2_TAB_COMMANDS: [(OverflowItem, Command, &str, &str); 3] = [
    (OverflowItem::NewTab, Command::Tab, "New tab", "⌘T"),
    (
        OverflowItem::Split,
        Command::SplitRight,
        "Split right",
        "⌘D",
    ),
    (OverflowItem::Split, Command::SplitDown, "Split down", "⇧⌘D"),
];

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

impl OverflowItem {
    /// The item's QA accessibility mark (`qa_ax::mark`), labelled with the
    /// text it paints; `None` unless the QA layer is on.
    fn qa_mark(self, label: &'static str) -> Option<AnyElement> {
        use crate::qa_ax::ids;
        match self {
            Self::Paste => crate::qa_ax::mark(ids::MENU_PASTE, label),
            Self::PasteImage => crate::qa_ax::mark(ids::MENU_PASTE_IMAGE, label),
            Self::SelectText => crate::qa_ax::mark(ids::MENU_SELECT_TEXT, label),
            Self::Copy => crate::qa_ax::mark(ids::MENU_COPY, label),
            Self::Find => crate::qa_ax::mark(ids::MENU_FIND, label),
            Self::CommandPalette => crate::qa_ax::mark(ids::MENU_COMMAND_PALETTE, label),
            Self::Reconnect => crate::qa_ax::mark(ids::MENU_RECONNECT, label),
            Self::CloseTab => crate::qa_ax::mark(ids::MENU_CLOSE_TAB, label),
            Self::Detach => crate::qa_ax::mark(ids::MENU_DETACH, label),
            Self::NewTab => crate::qa_ax::mark(ids::MENU_NEW_TAB, label),
            Self::Split => crate::qa_ax::mark(ids::MENU_SPLIT, label),
            Self::Pane1 => crate::qa_ax::mark(ids::MENU_PANE_1, label),
            Self::Pane2 => crate::qa_ax::mark(ids::MENU_PANE_2, label),
            Self::SplitDirection => crate::qa_ax::mark(ids::MENU_SPLIT_DIRECTION, label),
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
    let paste = [
        entry(OverflowItem::Paste, online, NOT_CONNECTED),
        entry(OverflowItem::PasteImage, online, NOT_CONNECTED),
    ];
    let reconnect = entry(OverflowItem::Reconnect, !online, ALREADY_CONNECTED);
    let select = entry(OverflowItem::SelectText, online, NOT_CONNECTED);
    let find = entry(OverflowItem::Find, state.find_available, FIND_UNAVAILABLE);
    let mut items = Vec::new();
    match state.kind {
        HerdrKind::Herdr1 => {
            items.extend(paste);
            items.push(select);
            if state.selecting {
                items.push(entry(OverflowItem::Copy, true, NOT_CONNECTED));
            }
            items.extend([
                find,
                entry(OverflowItem::CommandPalette, true, NOT_CONNECTED),
                reconnect,
                entry(OverflowItem::CloseTab, online, NOT_CONNECTED),
            ]);
        }
        HerdrKind::Herdr2 => {
            items.extend([
                entry(OverflowItem::NewTab, online, NOT_CONNECTED),
                entry(OverflowItem::Split, online, NOT_CONNECTED),
            ]);
            items.extend(paste);
            items.extend([
                reconnect,
                entry(OverflowItem::Detach, online, NOT_CONNECTED),
            ]);
            // Preserve selection and the existing pane controls. The view
            // draws pane controls only when the active tab has two panes.
            items.extend([select, entry(OverflowItem::Copy, online, NOT_CONNECTED)]);
            if state.find_available {
                items.push(find);
            }
            for item in [
                OverflowItem::Pane1,
                OverflowItem::Pane2,
                OverflowItem::SplitDirection,
            ] {
                items.push(entry(item, online, NOT_CONNECTED));
            }
        }
    }
    items
}

fn menu_entries_for_frame(state: &ChromeState, compact: bool) -> Vec<OverflowEntry> {
    let mut items = overflow_menu_items(state);
    if compact && state.kind == HerdrKind::Herdr1 {
        // Compact has no separate new-tab button.
        items.insert(
            0,
            entry(OverflowItem::NewTab, state.connected, NOT_CONNECTED),
        );
    }
    items
}

impl Smoke {
    /// The open ... menu, anchored under the tab row's right edge; `None`
    /// while closed.
    pub(crate) fn render_overflow_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.render_overflow_menu_at(None, cx)
    }

    pub(in crate::shell) fn render_overflow_menu_at(
        &mut self,
        compact_top: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.shell.frame.overflow_open {
            return None;
        }
        let state = self.chrome_state();
        let herdr2 = state.kind == HerdrKind::Herdr2;
        let catalog = herdr2.then(|| actions::catalog(&self.terminal));
        let split = herdr2
            && self
                .core_tabs()
                .iter()
                .any(|tab| tab.active && tab.panes.len() > 1);
        let mut rows = Vec::new();
        for menu_entry in menu_entries_for_frame(&state, compact_top.is_some()) {
            let item = menu_entry.item;
            if herdr2
                && !split
                && matches!(
                    item,
                    OverflowItem::Pane1 | OverflowItem::Pane2 | OverflowItem::SplitDirection
                )
            {
                continue;
            }
            if let Some(catalog) = &catalog {
                if matches!(
                    item,
                    OverflowItem::Paste | OverflowItem::Reconnect | OverflowItem::SelectText
                ) {
                    rows.push(menu_divider().into_any_element());
                }
                if matches!(item, OverflowItem::NewTab | OverflowItem::Split) {
                    for (_, command, label, keys) in HERDR2_TAB_COMMANDS
                        .into_iter()
                        .filter(|(row, ..)| *row == item)
                    {
                        let enabled = catalog
                            .iter()
                            .any(|row| row.info.command == command && row.available);
                        let command_entry = entry(
                            item,
                            enabled,
                            if state.connected {
                                COMMAND_UNAVAILABLE
                            } else {
                                NOT_CONNECTED
                            },
                        );
                        rows.push(overflow_row(
                            rows.len(),
                            command_entry,
                            label,
                            Some(command),
                            overflow_shortcut(&self.terminal, &self.actions, command, keys),
                            cx,
                        ));
                    }
                    continue;
                }
            }
            let label = if item == OverflowItem::Detach {
                DETACH_LABEL
            } else {
                item.label()
            };
            rows.push(overflow_row(rows.len(), menu_entry, label, None, None, cx));
        }
        Some(
            div()
                .absolute()
                .top(px(compact_top.unwrap_or(MENU_TOP)))
                .bottom_0()
                .right(px(MENU_RIGHT))
                .w(px(metric::MENU_WIDTH))
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("shell-overflow-menu")
                        .debug_selector(|| "shell-overflow-menu".into())
                        .min_h_0()
                        .overflow_y_scroll()
                        .occlude()
                        // A press outside the terminal column (the sidebar) closes it.
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.shell.frame.overflow_open = false;
                            cx.notify();
                        }))
                        .py(px(metric::MENU_PADDING))
                        .flex()
                        .flex_col()
                        .rounded(px(radius::POPOVER))
                        .shadow(vec![shadow::POPOVER.box_shadow()])
                        .bg(rgb(color::RAISED))
                        .text_color(rgb(color::TEXT))
                        .children(rows),
                )
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
        split_direction: Option<Command>,
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
            OverflowItem::Split => self.run_action(split_direction.unwrap_or(Command::SplitRight)),
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
            self.shell.shortcuts.status_detail_open = None;
            self.connect_terminal(cx);
        }
    }
}

fn menu_divider() -> gpui::Div {
    div()
        .flex_none()
        .h(px(metric::LINE))
        .my_1()
        .bg(rgb(DIVIDER_COLOR))
}

fn overflow_shortcut(
    terminal: &crate::terminal_view::TerminalView,
    actions: &actions::Actions,
    command: Command,
    ipad: &'static str,
) -> Option<String> {
    actions.first_shortcut(terminal, command, ipad)
}

fn overflow_row(
    index: usize,
    entry: OverflowEntry,
    label: &'static str,
    command: Option<Command>,
    keys: Option<String>,
    cx: &mut Context<Smoke>,
) -> AnyElement {
    let item = entry.item;
    let has_legacy_qa_mark = command != Some(Command::SplitDown);
    div()
        .id(("shell-overflow-item", index))
        .debug_selector(move || format!("shell-overflow-item-{label}"))
        .h(px(metric::MENU_ITEM))
        .px_4()
        .flex()
        .flex_none()
        .flex_col()
        .justify_center()
        .text_color(rgb(if entry.enabled {
            color::TEXT
        } else {
            color::DISABLED
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(label)
                .gap_2()
                .when_some(keys, |line, keys| {
                    line.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(if entry.enabled {
                                color::MUTED
                            } else {
                                color::DISABLED
                            }))
                            .child(keys),
                    )
                }),
        )
        .children(entry.reason.map(|reason| {
            div()
                .text_size(px(text::SECONDARY))
                .text_color(rgb(color::DISABLED))
                .child(reason)
        }))
        .when(has_legacy_qa_mark, |row| row.children(item.qa_mark(label)))
        .when(entry.enabled, |row| {
            row.on_click(cx.listener(move |this, _, window, cx| {
                this.shell.frame.overflow_open = false;
                this.run_overflow_item(item, command, window, cx);
                cx.notify();
            }))
        })
        .into_any_element()
}

#[cfg(test)]
#[path = "overflow_menu_tests.rs"]
mod tests;
