//! L9: L0 placement on the real screen, too-small window, keyboards.
//! Owner: L9.
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

#[derive(Debug, Default)]
pub(crate) struct FitState {}

/// Status bar items in the order they move to the details popover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatusItem {
    HardwareKeyboard,
    ModSections,
}

pub(crate) fn status_bar_overflow(_available_width: f32) -> Vec<StatusItem> {
    Vec::new()
}

impl Smoke {
    /// "Window too small" in place of the terminal; `None` draws as today.
    pub(crate) fn render_too_small(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }

    /// False while the window is too small: no size or input reaches the
    /// terminal.
    pub(crate) fn terminal_send_allowed(&self) -> bool {
        true
    }
}
