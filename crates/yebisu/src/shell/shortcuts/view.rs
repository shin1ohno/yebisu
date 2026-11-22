//! The Shortcuts popover's view (`Shortcuts.dc.html`). Owner: L6.
//!
//! Anchored 12 pt above the status bar's right end; a dim backdrop covers
//! everything above the status bar and closes it on tap, so the status bar's
//! Shortcuts stays reachable and closes it too. Title and footer stay fixed;
//! the rows scroll when the window is too short, so the popover's top never
//! goes above 16 pt.
use super::{
    POPOVER_MARGIN, PopoverAction, ROW_HEIGHT, ShortcutGroup, ShortcutRow, canvas_rows, grouped,
    header_hint, popover_columns, popover_rows, popover_width,
};
use crate::{
    app::Smoke,
    layout::STATUS_BAR_HEIGHT,
    shell::{
        style::{color, metric, radius, shadow, text, tint, weight},
        tab_row::icons::Icon,
    },
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, relative, rgb,
};

const ANCHOR_GAP: f32 = 12.;
const GROUP_HEADER_HEIGHT: f32 = 28.;
const FOOTER_HEIGHT: f32 = 48.;
// Shortcuts.dc.html:63.
const ARROW_RIGHT: f32 = 52.;
const ARROW_HEIGHT: f32 = 10.;
// Shortcuts.dc.html:44 renders the direction names on two lines.
const FOCUS_LABEL_LINE: f32 = 16.;
pub(crate) const TITLE: &str = "Keyboard shortcuts";
pub(crate) const CHANGE_IN_SETTINGS: &str = "Change in Settings › Input";

/// "Command key: Local · Shortcut priority: App" (the canvas footer).
pub(crate) fn settings_line(command_control: bool, terminal_priority: bool) -> String {
    let command = if command_control { "Ctrl" } else { "Local" };
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
        let groups = grouped(popover_rows(&self.terminal, &state, &self.actions));
        let herdr_prefix = self.actions.herdr_prefix;
        let hint = header_hint(&self.terminal, &self.actions);
        let notice = herdr_prefix
            .then(|| {
                self.actions
                    .host_keys
                    .shortcut_notice(&self.terminal, self.actions.use_server_keybindings)
            })
            .flatten();
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
                .child(group_header(group.title()));
            let expanded = self.shell.shortcuts.expanded == Some(group);
            for (action, row) in canvas_rows(group, rows, expanded, herdr_prefix) {
                column = column.child(self.shortcut_row(action, row, cx));
            }
            if group == ShortcutGroup::App {
                column = column.child(group_header("EDIT"));
                let edit = [
                    (
                        PopoverAction::Copy,
                        "Copy",
                        "⌘C",
                        self.handoff.selection.is_some(),
                    ),
                    (PopoverAction::Paste, "Paste", "⌘V", state.connected),
                ];
                for (action, label, keys, enabled) in edit {
                    column = column.child(self.shortcut_row(
                        action,
                        ShortcutRow {
                            label: label.into(),
                            keys: keys.into(),
                            enabled,
                        },
                        cx,
                    ));
                }
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
            .rounded(px(radius::POPOVER))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
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
                            .text_size(px(text::SHEET_TITLE))
                            .font_weight(weight::BOLD)
                            .child(TITLE),
                    )
                    .when(self.shell.fit.hardware_keyboard, |header| {
                        header.child(
                            div()
                                .debug_selector(|| "shell-shortcuts-hold-hint".into())
                                .text_size(px(text::SECONDARY))
                                .text_color(rgb(color::MUTED))
                                .child(hint),
                        )
                    }),
            )
            .children(notice.map(|message| {
                div()
                    .px_5()
                    .pb_2()
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(message)
            }))
            .child(body)
            .child(
                div()
                    .absolute()
                    .right(px(ARROW_RIGHT))
                    .bottom(px(-ARROW_HEIGHT))
                    .debug_selector(|| "shell-shortcuts-arrow".into())
                    .child(Icon::PopoverArrow.element(color::RAISED)),
            )
            .child(
                div()
                    .h(px(FOOTER_HEIGHT))
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(color::SEPARATOR))
                    .text_size(px(text::SECONDARY))
                    .text_color(rgb(color::MUTED))
                    .child(div().min_w_0().truncate().child(footer))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("shell-shortcuts-settings")
                            .h(px(44.))
                            .px_1()
                            .flex()
                            .items_center()
                            .text_color(rgb(color::ACCENT))
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
                .bg(gpui::Rgba::from(tint::POPOVER_SCRIM))
                .occlude()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.close_shortcuts(window, cx);
                }))
                .child(popover)
                .into_any_element(),
        )
    }

    fn shortcut_row(
        &self,
        action: PopoverAction,
        row: ShortcutRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = row.label.clone();
        let enabled = row.enabled;
        let focus_directions =
            !self.actions.herdr_prefix && action == PopoverAction::Focus(ShortcutGroup::Panes);
        div()
            .id(gpui::ElementId::Name(format!("shell-shortcut-{id}").into()))
            .h(px(ROW_HEIGHT))
            .w(relative(1.))
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded(px(radius::CONTROL))
            .text_color(rgb(if enabled {
                color::TEXT
            } else {
                color::DISABLED
            }))
            .child(
                div()
                    .min_w_0()
                    .when(focus_directions, |label| {
                        label.line_height(px(FOCUS_LABEL_LINE))
                    })
                    .when(!focus_directions, |label| label.truncate())
                    .child(row.label),
            )
            .child(
                div()
                    .when(self.actions.herdr_prefix, |key| key.min_w_0().truncate())
                    .when(!self.actions.herdr_prefix, |key| key.flex_none())
                    .text_color(rgb(if enabled {
                        color::MUTED
                    } else {
                        color::DISABLED
                    }))
                    .child(row.keys),
            )
            .when(enabled, |row| {
                row.active(|row| row.bg(gpui::Rgba::from(tint::SELECTED)))
                    .on_click(cx.listener(move |this, _, window, cx| match action {
                        PopoverAction::Catalog(command) => {
                            this.tap_shortcut_row(command, window, cx);
                        }
                        PopoverAction::Focus(group) => {
                            let expanded = &mut this.shell.shortcuts.expanded;
                            *expanded = (*expanded != Some(group)).then_some(group);
                            cx.notify();
                        }
                        PopoverAction::Settings => this.open_settings_from_shortcut(window, cx),
                        PopoverAction::Copy => {
                            this.copy_selection(cx);
                            this.close_shortcuts(window, cx);
                        }
                        PopoverAction::Paste => {
                            this.close_shortcuts(window, cx);
                            this.start_paste(false, cx);
                        }
                    }))
            })
    }
}

fn group_header(label: &'static str) -> gpui::Div {
    div()
        .h(px(GROUP_HEADER_HEIGHT))
        .px(px(metric::INLINE_GAP))
        .flex()
        .items_center()
        .text_size(px(text::SMALL))
        .font_weight(weight::SEMIBOLD)
        .text_color(rgb(color::MUTED))
        .child(label)
}
