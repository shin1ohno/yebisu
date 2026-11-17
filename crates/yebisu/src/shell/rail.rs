//! L4: the 72 pt rail. Owner: L4.
use super::{ChromeState, EntryPoint};
use crate::app::Smoke;
use gpui::{AnyElement, Context};

#[derive(Debug, Default)]
pub(crate) struct RailState {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    #[default]
    Full,
    Rail,
}

/// The mode for a window width and the user's choice (Q2: collapse last).
pub(crate) fn sidebar_mode(_window_width: f32, user: SidebarMode) -> SidebarMode {
    user
}

pub(crate) fn rail_entry_points(_state: &ChromeState) -> Vec<EntryPoint> {
    Vec::new()
}

impl Smoke {
    /// `Command::ToggleSidebar` (stays a local command, PR 44's oracle).
    /// Wave 0 keeps today's behaviour: hide or show the board.
    pub(crate) fn toggle_sidebar(&mut self) {
        self.actions.board_hidden ^= true;
    }

    pub(crate) fn render_rail(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
