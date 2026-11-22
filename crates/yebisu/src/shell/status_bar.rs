//! L2a: the one-line status bar. Owner: L2a.
//! Text from other streams comes through their own functions:
//! L5 `edit_bar::paused_typing_status`, L9 `fit::status_bar_overflow`,
//! L9 `mods::mods_item`, L6 `Smoke::open_shortcuts`.
//!
//! One line: "connection · Herdr 1/2 · state", then Reconnect (disconnected
//! only), "Expand sidebar ⌘B" (sidebar hidden only), and Shortcuts at the
//! right. A long connection name is cut with an ellipsis on screen and kept
//! whole in the accessibility label (S-8).
use super::{
    ChromeState, HerdrKind,
    fit::{self, STATUS_GAP, StatusItem},
    mods,
    style::{color, metric, radius, shadow, tint},
    tab_row::icons::Icon,
};
use crate::{
    app::Smoke,
    hosts::HostName,
    layout::{STATUS_BAR_HEIGHT, Size},
};
use gpui::{AnyElement, Context, Role, Window, deferred, div, prelude::*, px, rgb};
use sheprd_commands::Command;

pub(crate) const SHOW_SIDEBAR: &str = "Expand sidebar ⌘B";
pub(crate) const SHORTCUTS: &str = "Shortcuts";
pub(crate) const RECONNECT: &str = "Reconnect";
/// L9's optional item: a hardware keyboard is in use (Main.dc.html).
pub(crate) const HARDWARE_KEYBOARD: &str = "Hardware keyboard";

/// The bar's side padding (`px_3`, both sides) and its status dot.
const BAR_PADDING: f32 = 24.0;
const DOT_WIDTH: f32 = metric::SMALL_DOT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StatusDetailScope {
    selection: u64,
    generation: u64,
    connected: bool,
}
// Main.dc.html:185–189 and Shortcuts.dc.html:66.
const ICON_GAP: f32 = 6.;
const SHORTCUTS_WIDTH: f32 = 104.;
const SHORTCUTS_HEIGHT: f32 = 36.;
/// The connection line at its shortest: a few characters and the ellipsis.
const MIN_LINE_WIDTH: f32 = 96.0;
/// Width per character of a 13 pt chip label, and a chip's `px_2` padding.
const CHIP_CHAR_WIDTH: f32 = 7.0;
const CHIP_PADDING: f32 = 16.0;
/// A docked keyboard taller than this is the software keyboard; the bar a
/// hardware keyboard leaves at the bottom edge is shorter [inference].
const SOFTWARE_KEYBOARD_MIN_HEIGHT: f32 = 120.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StatusBarModel {
    pub(crate) connection: String,
    pub(crate) kind: HerdrKind,
    pub(crate) state: String,
    pub(crate) show_reconnect: bool,
    pub(crate) show_sidebar_hint: bool,
    pub(crate) accessibility_label: String,
}

pub(crate) fn kind_label(kind: HerdrKind) -> &'static str {
    match kind {
        HerdrKind::Herdr1 => "Herdr 1",
        HerdrKind::Herdr2 => "Herdr 2",
    }
}

/// Terminal and daemon text is untrusted: line breaks and other control
/// characters become spaces so the bar stays one line.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
pub(crate) fn status_bar_model(state: &ChromeState, connection: &str) -> StatusBarModel {
    status_bar_model_for_host(state, &HostName::new(connection))
}

fn status_bar_model_for_host(state: &ChromeState, connection: &HostName) -> StatusBarModel {
    let connection = one_line(connection.display());
    let status = if state.connected {
        "Connected"
    } else {
        "Disconnected"
    };
    let kind = kind_label(state.kind);
    StatusBarModel {
        accessibility_label: format!("{connection}, {kind}, {status}"),
        connection,
        kind: state.kind,
        state: status.to_owned(),
        show_reconnect: !state.connected,
        // While the width forces the rail, "Show sidebar" could not show it.
        show_sidebar_hint: state.sidebar_hidden && !state.sidebar_forced,
    }
}

/// The terminal's own status text (`Smoke::terminal_status`: "Paste queued",
/// "Text input paused · tap terminal to type", errors) that only the retired
/// toolbar row showed (L2b). Shown after the state unless it repeats it.
pub(crate) fn status_detail(model: &StatusBarModel, detail: &str) -> Option<String> {
    let detail = one_line(detail.trim());
    (!detail.is_empty() && !detail.eq_ignore_ascii_case(&model.state)).then_some(detail)
}

fn chip_width(label: &str) -> f32 {
    label.chars().count() as f32 * CHIP_CHAR_WIDTH + CHIP_PADDING
}

/// Items the bar moves to the details at `bar_width` (L9 order,
/// `fit::status_bar_overflow_with`): the room left beside the items that
/// always stay (status dot, the connection line at its shortest, Reconnect,
/// Mods, Show sidebar, Shortcuts) goes to the optional items, Hardware
/// keyboard first, then the M5 sections (`m5_width`, 0 with no section).
pub(crate) fn status_bar_details(
    bar_width: f32,
    model: &StatusBarModel,
    mods: Option<&str>,
    m5_width: f32,
) -> Vec<StatusItem> {
    let chips = [
        Some(SHORTCUTS),
        model.show_reconnect.then_some(RECONNECT),
        model.show_sidebar_hint.then_some(SHOW_SIDEBAR),
        mods,
    ];
    let stay = chips
        .into_iter()
        .flatten()
        .map(|label| STATUS_GAP + chip_width(label))
        .sum::<f32>()
        + BAR_PADDING
        + DOT_WIDTH
        + STATUS_GAP
        + MIN_LINE_WIDTH;
    fit::status_bar_overflow_with(bar_width - stay, m5_width)
}

/// The visible text of the bar's left side.
pub(crate) fn status_line(model: &StatusBarModel) -> String {
    format!(
        "{} · {} · {}",
        model.connection,
        kind_label(model.kind),
        model.state
    )
}

impl Smoke {
    pub(in crate::shell) fn current_status_bar_model(&self, state: &ChromeState) -> StatusBarModel {
        status_bar_model_for_host(
            state,
            &self.setup.host_name(&self.terminal.reconnect.target),
        )
    }

    /// L9: a press only a hardware keyboard sends marks one in use
    /// (`fit::watch_hardware_keys`); the software keyboard docked on screen
    /// clears it.
    fn note_hardware_keyboard(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.watch_hardware_keys(cx);
        let viewport = window.viewport_size();
        let size = Size::new(f32::from(viewport.width), f32::from(viewport.height));
        let ime = self.platform_insets(window).ime;
        let docked = fit::docked_keyboard_height(size, fit::ime_keyboard(size, &ime));
        if docked > SOFTWARE_KEYBOARD_MIN_HEIGHT {
            self.shell.fit.hardware_keyboard = false;
        }
    }

    pub(crate) fn render_status_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.chrome_state();
        // L9: the mod regions' 30 s fade runs on this line state.
        self.observe_disconnect(state.connected, cx);
        self.note_hardware_keyboard(window, cx);
        let model = self.current_status_bar_model(&state);
        let mods = mods::mods_item(self.shell.frame.withdrawn);
        let layout = self.frame_layout(window);
        let software_keyboard = layout.key_band.is_some();
        let details = status_bar_details(
            layout.status_bar.width,
            &model,
            mods.as_deref(),
            layout.m5.width,
        );
        let hardware_keyboard =
            self.shell.fit.hardware_keyboard && !details.contains(&StatusItem::HardwareKeyboard);
        let selection = self.selection_status(usize::MAX);
        let detail = status_detail(&model, &self.terminal_status());
        let scope = StatusDetailScope {
            selection: self.board.selection,
            generation: self.terminal.link.generation,
            connected: state.connected,
        };
        if detail.is_none() || self.shell.shortcuts.status_detail_open != Some(scope) {
            self.shell.shortcuts.status_detail_open = None;
        }
        let show_details = self.shell.shortcuts.status_detail_open.is_some();
        let shortcuts_selected = self.shell.shortcuts.open;
        Some(
            div()
                .id("shell-status-bar")
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .gap_3()
                .px_3()
                .w_full()
                .h(px(STATUS_BAR_HEIGHT))
                .bg(rgb(color::SURFACE))
                .text_color(rgb(color::TEXT))
                .child(div().flex_none().size(px(DOT_WIDTH)).rounded_full().bg(rgb(
                    if state.connected {
                        color::GREEN
                    } else {
                        color::RED
                    },
                )))
                .child(
                    div()
                        .id("shell-status-line")
                        .min_w_0()
                        .flex()
                        .flex_shrink(1.)
                        .items_center()
                        .gap_1()
                        .debug_selector(|| "shell-status-line".into())
                        .aria_label(if detail.is_some() {
                            format!(
                                "{}, connection details available",
                                model.accessibility_label
                            )
                        } else {
                            model.accessibility_label.clone()
                        })
                        .child(div().min_w_0().truncate().child(status_line(&model)))
                        .when(detail.is_some(), |line| {
                            line.role(Role::Button)
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(rgb(color::MUTED))
                                        .debug_selector(|| "shell-status-detail-mark".into())
                                        .child(if show_details { "▾" } else { "▸" }),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.shell.shortcuts.status_detail_open =
                                        (!show_details).then_some(scope);
                                    cx.notify();
                                }))
                        })
                        .when(show_details, |line| {
                            // Observe the press without taking it from the terminal,
                            // including a tap under the informational panel.
                            line.on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.shell.shortcuts.status_detail_open = None;
                                cx.notify();
                            }))
                        }),
                )
                .when_some(selection, |bar, selection| {
                    bar.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(color::MUTED))
                            .debug_selector(|| "shell-selection-status".into())
                            .child(selection),
                    )
                })
                .when_some(detail.filter(|_| show_details), |bar, detail| {
                    bar.child(deferred(
                        div()
                            .debug_selector(|| "shell-status-detail".into())
                            .absolute()
                            .left_3()
                            .bottom(px(STATUS_BAR_HEIGHT))
                            .max_w(px((layout.status_bar.width - BAR_PADDING).max(0.)))
                            .px_3()
                            .py_2()
                            .bg(rgb(color::RAISED))
                            .rounded(px(radius::CONTROL))
                            .shadow(vec![shadow::POPOVER.box_shadow()])
                            .child(detail),
                    ))
                })
                .when(model.show_reconnect, |view| {
                    view.child(
                        chip("shell-status-reconnect", RECONNECT, None)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reconnect_from_chrome(cx);
                                cx.notify();
                            }))
                            .children(crate::qa_ax::mark(
                                crate::qa_ax::ids::STATUS_RECONNECT,
                                RECONNECT,
                            )),
                    )
                })
                .when(hardware_keyboard, |view| {
                    view.child(chip(
                        "shell-status-hardware-keyboard",
                        HARDWARE_KEYBOARD,
                        Some(Icon::Keyboard),
                    ))
                })
                .when(software_keyboard, |view| {
                    view.child(chip(
                        "shell-status-software-keyboard",
                        "Software keyboard",
                        Some(Icon::Keyboard),
                    ))
                })
                .child(div().flex_1())
                // L9: Mods sits beside Shortcuts (ModsMax.dc.html) and opens
                // the withdrawn regions' list in overlay O.
                .children(self.render_mods_status(&layout.withdrawn, cx))
                .when(model.show_sidebar_hint, |view| {
                    view.child(
                        chip("shell-status-show-sidebar", SHOW_SIDEBAR, None).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.toggle_sidebar();
                                cx.notify();
                            }),
                        ),
                    )
                })
                .when(!software_keyboard, |view| {
                    view.child(
                        chip("shell-status-shortcuts", SHORTCUTS, Some(Icon::Keyboard))
                            .w(px(SHORTCUTS_WIDTH))
                            .h(px(SHORTCUTS_HEIGHT))
                            .justify_center()
                            .px_0()
                            .rounded(px(radius::CONTROL))
                            .when(shortcuts_selected, |chip| {
                                chip.bg(gpui::Rgba::from(tint::SELECTED))
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                // L6 fills the popover; until then Shortcuts opens the
                                // command palette (plan v2 5.2, L2a column).
                                if !this.toggle_shortcuts(window, cx)
                                    && let Err(e) = this.run_action(Command::Palette)
                                {
                                    this.terminal.live.status = e.to_string();
                                }
                                cx.notify();
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}

fn chip(id: &'static str, label: &'static str, icon: Option<Icon>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .px_2()
        .whitespace_nowrap()
        .text_color(rgb(color::MUTED))
        .gap(px(ICON_GAP))
        .children(icon.map(Icon::muted))
        .child(label)
}

#[cfg(test)]
#[path = "status_bar_tests.rs"]
mod tests;
