//! L2b: the Simulator-only QA strip (plan v2 5.3). Owner: L2b.
//!
//! The six QA bands that sat between the toolbar and the terminal live here
//! now, at the bottom of the terminal column above the status bar. The
//! controls themselves are unchanged (`input_qa_controls`,
//! `handoff_qa_controls`, `native_edit_qa_controls`, `edit_menu_qa_controls`,
//! `sh1_156_qa_controls`, `sh1_152_controls`, `p13_qa_controls`); each one
//! still draws only with its own QA environment variable. Outside a
//! Simulator build the strip is never built, so a product build has none.
use crate::app::Smoke;
use gpui::{AnyElement, Context};

/// The QA variables that turn on a control in the strip.
pub(crate) const QA_ENV: [&str; 7] = [
    "YEBISU_INPUT_QA",
    "YEBISU_HANDOFF_QA",
    "YEBISU_NATIVE_QA",
    "YEBISU_EDIT_MENU_QA",
    "YEBISU_SH1_152_CASE",
    "YEBISU_SH1_156_QA",
    "YEBISU_P13_QA",
];

/// Whether this build may draw the strip: only a Simulator build.
pub(crate) const STRIP_BUILD: bool = cfg!(all(target_os = "ios", target_abi = "sim"));

/// The rule: a Simulator build with at least one QA variable set.
pub(crate) fn strip_allowed(simulator: bool, env_set: impl Fn(&str) -> bool) -> bool {
    simulator && QA_ENV.iter().any(|name| env_set(name))
}

/// True only in a Simulator build with a QA environment variable set.
pub(crate) fn qa_strip_enabled() -> bool {
    strip_allowed(STRIP_BUILD, |name| std::env::var_os(name).is_some())
}

impl Smoke {
    /// A press on the strip focuses the terminal first, at mouse down, as it
    /// did when these controls sat inside the terminal body, which tracks
    /// `terminal_focus` (GPUI moves a tracked focus on mouse down). Without
    /// it a QA control that focuses the terminal itself does so after its
    /// own keyboard request: SH1-153 case 9 (a press right after a blur)
    /// found the text input view as first responder although the case had
    /// dismissed the keyboard (Integration 2, run 44220412031).
    pub(crate) fn qa_strip_press(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.terminal.open && !self.input_modal() && !self.terminal_focus.is_focused(window) {
            self.terminal_focus.focus(window, cx);
        }
    }

    pub(crate) fn render_qa_strip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !qa_strip_enabled() {
            return None;
        }
        self.qa_strip_controls(cx)
    }

    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    fn qa_strip_controls(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui::{div, prelude::*};
        let controls: Vec<AnyElement> = [
            self.input_qa_controls(cx),
            self.p13_qa_controls(cx),
            self.handoff_qa_controls(cx),
            self.native_edit_qa_controls(cx),
            self.edit_menu_qa_controls(cx),
            self.sh1_156_qa_controls(cx),
            self.sh1_152_controls(cx),
        ]
        .into_iter()
        .flatten()
        .collect();
        if controls.is_empty() {
            return None;
        }
        Some(
            div()
                .id("shell-qa-strip")
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.qa_strip_press(window, cx)),
                )
                .flex_none()
                .flex()
                .flex_col()
                .w_full()
                .children(controls)
                .into_any_element(),
        )
    }

    #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
    fn qa_strip_controls(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}

#[cfg(test)]
#[path = "qa_strip_tests.rs"]
mod tests;
