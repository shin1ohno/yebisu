//! L5: the edit bar shown only while selecting. Owner: L5.
use super::{ChromeState, EntryPoint};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window};

#[derive(Debug, Default)]
pub(crate) struct EditBarState {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditBarItem {
    Copy,
    Paste,
    PasteImage,
    Cancel,
}

pub(crate) fn edit_bar_items(_selecting: bool) -> Vec<EditBarItem> {
    Vec::new()
}

/// The status bar text while typing is paused for a selection.
pub(crate) fn paused_typing_status(_selecting: bool) -> Option<&'static str> {
    None
}

pub(crate) fn edit_bar_entry_points(_state: &ChromeState) -> Vec<EntryPoint> {
    Vec::new()
}

impl Smoke {
    pub(crate) fn render_edit_bar(&mut self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }

    /// Cancel: clears the selection and resumes typing (`input.rs:153`).
    pub(crate) fn cancel_selection(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}
