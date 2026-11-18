//! The Shortcuts popover's view (`Shortcuts.dc.html`). Owner: L6.
//!
//! Anchored 12 pt above the status bar's right end; a dim backdrop covers
//! everything above the status bar and closes it on tap, so the status bar's
//! Shortcuts stays reachable and closes it too. Title and footer stay fixed;
//! the rows scroll when the window is too short, so the popover's top never
//! goes above 16 pt.
use super::{
    POPOVER_MARGIN, ROW_HEIGHT, ShortcutRow, grouped, popover_columns, popover_rows, popover_width,
};
use crate::{app::Smoke, layout::STATUS_BAR_HEIGHT};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, relative, rgb,
    rgba,
};
use sheprd_commands::Command;

const ANCHOR_GAP: f32 = 12.;
const GROUP_HEADER_HEIGHT: f32 = 28.;
const FOOTER_HEIGHT: f32 = 48.;
const TEXT: u32 = 0xd8dee9;
const MUTED: u32 = 0xacb2bb;
const DISABLED: u32 = 0x6b7080;
pub(crate) const TITLE: &str = "Keyboard shortcuts";
pub(crate) const HINT: &str = "Hold ⌘ anywhere to see this list. Tap a row to run it.";
pub(crate) const CHANGE_IN_SETTINGS: &str = "Change in Settings › Input";

/// "Command key: Local · Shortcut priority: App" (the canvas footer).
pub(crate) fn settings_line(command_control: bool, terminal_priority: bool) -> String {
    let command = if command_control { "Control" } else { "Local" };
    let priority = if terminal_priority { "Terminal" } else { "App" };
    format!("Command key: {command} · Shortcut priority: {priority}")
}

impl Smoke {
    pub(crate) fn render_shortcuts_popover(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.shortcuts_shown() {
            return None;
        }
        let viewport = window.viewport_size();
        let width = popover_width(f32::from(viewport.width));
        let columns = popover_columns(width);
        let state = self.popover_chrome_state();
        let groups = grouped(popover_rows(&self.terminal, &state));
        let footer = settings_line(
            self.terminal.input.options.command_control,
            self.actions.terminal_priority,
        );
        // The top stays 16 pt below the window's top; the rows scroll.
        let max_height =
            (f32::from(viewport.height) - STATUS_BAR_HEIGHT - ANCHOR_GAP - POPOVER_MARGIN)
                .max(ROW_HEIGHT);
        let mut columns_of = Vec::new();
        for (group, rows) in groups {
            let mut column = div()
                .when(columns > 1, |view| view.flex_1().min_w_0())
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(GROUP_HEADER_HEIGHT))
                        .px_2()
                        .flex()
                        .items_center()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(MUTED))
                        .child(group.title()),
                );
            for (command, row) in rows {
                column = column.child(self.shortcut_row(command, row, cx));
            }
            columns_of.push(column);
        }
        let body = div()
            .id("shell-shortcuts-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .pb_2()
            .flex()
            .when(columns == 1, |view| view.flex_col())
            .gap_x_4()
            .children(columns_of);
        let popover = div()
            .id("shell-shortcuts-popover")
            .absolute()
            .right(px(ANCHOR_GAP))
            .bottom(px(ANCHOR_GAP))
            .w(px(width))
            .max_h(px(max_height))
            .flex()
            .flex_col()
            .pt_4()
            .rounded(px(14.))
            .bg(rgb(0x2b2933))
            .text_color(rgb(TEXT))
            .occlude()
            // A tap inside the popover but off a row keeps it open.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px_5()
                    .pb_3()
                    .flex()
                    .flex_wrap()
                    .items_baseline()
                    .justify_between()
                    .gap_x_4()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(TITLE),
                    )
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(HINT)),
            )
            .child(body)
            .child(
                div()
                    .h(px(FOOTER_HEIGHT))
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(0x3b3944))
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(div().min_w_0().truncate().child(footer))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("shell-shortcuts-settings")
                            .h(px(44.))
                            .px_1()
                            .flex()
                            .items_center()
                            .text_color(rgb(0x88c0d0))
                            .child(CHANGE_IN_SETTINGS)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.shell.shortcuts.open = false;
                                this.settings_page = crate::settings_pages::Page::Input;
                                this.begin_settings_modal(window, cx);
                                cx.notify();
                            })),
                    ),
            );
        Some(
            div()
                .id("shell-shortcuts-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom(px(STATUS_BAR_HEIGHT))
                .bg(rgba(0x10141973))
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shell.shortcuts.open = false;
                    cx.notify();
                }))
                .child(popover)
                .into_any_element(),
        )
    }

    fn shortcut_row(
        &self,
        command: Command,
        row: ShortcutRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = row.label.clone();
        div()
            .id(gpui::ElementId::Name(format!("shell-shortcut-{id}").into()))
            .h(px(ROW_HEIGHT))
            .w(relative(1.))
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded(px(8.))
            .text_color(rgb(if row.enabled { TEXT } else { DISABLED }))
            .child(div().min_w_0().truncate().child(row.label))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(if row.enabled { MUTED } else { DISABLED }))
                    .child(row.keys),
            )
            // `tap_shortcut_row` refuses a disabled row itself.
            .on_click(cx.listener(move |this, _, window, cx| {
                this.tap_shortcut_row(command, window, cx);
            }))
    }
}
