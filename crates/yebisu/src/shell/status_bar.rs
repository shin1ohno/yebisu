//! L2a: the one-line status bar. Owner: L2a.
//! Text from other streams comes through their own functions:
//! L5 `edit_bar::paused_typing_status`, L9 `fit::status_bar_overflow`,
//! L9 `mods::mods_item`, L6 `Smoke::open_shortcuts`.
//!
//! One line: "connection · Herdr 1/2 · state", then Reconnect (disconnected
//! only), "Show sidebar ⌘B" (sidebar hidden only), and Shortcuts at the
//! right. A long connection name is cut with an ellipsis on screen and kept
//! whole in the accessibility label (S-8).
use super::{ChromeState, HerdrKind, edit_bar, mods};
use crate::{app::Smoke, layout::STATUS_BAR_HEIGHT};
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};
use sheprd_commands::Command;

pub(crate) const SHOW_SIDEBAR: &str = "Show sidebar ⌘B";
pub(crate) const SHORTCUTS: &str = "Shortcuts";
pub(crate) const RECONNECT: &str = "Reconnect";

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

pub(crate) fn status_bar_model(state: &ChromeState, connection: &str) -> StatusBarModel {
    let connection = one_line(connection);
    let status = edit_bar::paused_typing_status(state.selecting).unwrap_or(if state.connected {
        "Connected"
    } else {
        "Disconnected"
    });
    let kind = kind_label(state.kind);
    StatusBarModel {
        accessibility_label: format!("{connection}, {kind}, {status}"),
        connection,
        kind: state.kind,
        state: status.to_owned(),
        show_reconnect: !state.connected,
        show_sidebar_hint: state.sidebar_hidden,
    }
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
    pub(crate) fn render_status_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.chrome_state();
        let model = status_bar_model(&state, &self.terminal.reconnect.target);
        let _ = window;
        let mods = mods::mods_item(self.shell.frame.withdrawn);
        let line = status_line(&model);
        Some(
            div()
                .id("shell-status-bar")
                .flex()
                .flex_none()
                .items_center()
                .gap_3()
                .px_3()
                .w_full()
                .h(px(STATUS_BAR_HEIGHT))
                .overflow_hidden()
                .bg(rgb(0x1c1c22))
                .text_sm()
                .text_color(rgb(0xd8dee9))
                .child(
                    div()
                        .flex_none()
                        .size(px(8.))
                        .rounded_full()
                        .bg(rgb(if state.connected { 0xa3be8c } else { 0xbf616a })),
                )
                .child(
                    div()
                        .id("shell-status-line")
                        .min_w_0()
                        .flex_shrink(1.)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(line),
                )
                .when(model.show_reconnect, |view| {
                    view.child(
                        chip("shell-status-reconnect", RECONNECT).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.reconnect_from_chrome(cx);
                                cx.notify();
                            },
                        )),
                    )
                })
                .children(mods.map(|text| div().flex_none().whitespace_nowrap().child(text)))
                .child(div().flex_1())
                .when(model.show_sidebar_hint, |view| {
                    view.child(chip("shell-status-show-sidebar", SHOW_SIDEBAR).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.toggle_sidebar();
                            cx.notify();
                        }),
                    ))
                })
                .child(
                    chip("shell-status-shortcuts", SHORTCUTS).on_click(cx.listener(
                        |this, _, _, cx| {
                            // L6 fills the popover; until then Shortcuts opens the
                            // command palette (plan v2 5.2, L2a column).
                            if !this.open_shortcuts(cx)
                                && let Err(e) = this.run_action(Command::Palette)
                            {
                                this.terminal.live.status = e.to_string();
                            }
                            cx.notify();
                        },
                    )),
                )
                .into_any_element(),
        )
    }
}

fn chip(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .px_2()
        .whitespace_nowrap()
        .text_color(rgb(0xacb2bb))
        .child(label)
}

#[cfg(test)]
#[path = "status_bar_tests.rs"]
mod tests;
