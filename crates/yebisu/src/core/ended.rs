//! Herdr 2 panes whose program ended. One pane's program ending never ends
//! the session: as the desktop client does (herdr-gpui
//! `core_terminals/tabs.rs` `poll_core_panes`), an ended pane in a split
//! closes and its neighbour takes its place, while the last pane of a tab
//! stays with its final screen. A terminal that finished before it could be
//! attached has nothing left to show, so its pane goes and a tab left without
//! panes closes, as herdr-app does (`src/terminal.rs` "It ended between being
//! listed and being attached").
use super::store::{Saved, Workspace};
use sheprd_backend::TerminalId;

/// Whether the core refused an attach because the terminal's program had
/// already finished. Any other refusal or a lost link stays an error.
pub(super) fn finished(error: &sheprd_core_client::Error) -> bool {
    matches!(
        error,
        sheprd_core_client::Error::Refused { kind: Some(kind), .. } if kind == "finished"
    )
}

/// Takes pane `index` out of `saved`. The pane before it (left or above)
/// is in use after when the removed one was, else the first.
pub(super) fn remove_pane(saved: &mut Saved, index: usize) {
    if index >= saved.ids.len() {
        return;
    }
    saved.ids.remove(index);
    if index < saved.active || (index == saved.active && index > 0) {
        saved.active -= 1;
    }
    saved.active = saved.active.min(saved.ids.len().saturating_sub(1));
}

/// Takes `id` out of the tab holding it; a tab left without panes closes and
/// the tab sliding into its place, or else the last, is in use after.
pub(super) fn remove_terminal(workspace: &mut Workspace, id: &TerminalId) -> bool {
    let Some((tab, pane)) = workspace.tabs.iter().enumerate().find_map(|(tab, value)| {
        value
            .layout
            .ids
            .iter()
            .position(|pane| pane == id)
            .map(|pane| (tab, pane))
    }) else {
        return false;
    };
    remove_pane(&mut workspace.tabs[tab].layout, pane);
    if workspace.tabs[tab].layout.ids.is_empty() {
        workspace.tabs.remove(tab);
        if tab < workspace.active {
            workspace.active -= 1;
        }
        workspace.active = workspace.active.min(workspace.tabs.len().saturating_sub(1));
    }
    true
}

#[cfg(test)]
#[path = "ended_tests.rs"]
mod tests;
