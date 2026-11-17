//! L2a: the two-column frame. Owner: L2a (docs/layout-contracts.md).
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

/// L2a's view state (column widths, sidebar header button, ...).
#[derive(Debug, Default)]
pub(crate) struct FrameState {}

impl Smoke {
    /// Wraps the terminal screen built by `render_terminal` in the frame:
    /// sidebar column (L3 list or the board, L2b footer, L4 rail), tab row,
    /// find bar, terminal column, L5 edit bar, L2b QA strip, status bar.
    /// Wave 0: returns `body` unchanged, so nothing on screen changes.
    pub(crate) fn shell_frame(
        &mut self,
        body: AnyElement,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        body
    }

    /// Whether the legacy board stays in the terminal screen's band list.
    /// L2a returns false once the sidebar column hosts the board.
    pub(crate) fn board_in_band(&self) -> bool {
        true
    }
}
