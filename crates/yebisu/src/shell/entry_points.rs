//! L2a: the entry-point model behind `chrome_entry_points_cover_required_set`.
//! Owner: L2a. Other streams contribute through their own functions:
//! L2b `footer::footer_entry_points`, L4 `rail::rail_entry_points`,
//! L5 `edit_bar::edit_bar_entry_points`, L6 `shortcuts::shortcuts_entry_points`.
//!
//! Plan v2 5.2, L2a column: the legacy toolbar and bands keep their entry
//! points (5.1 rule 1: L2a only adds places), and L2a adds the tab row, the
//! ... menu, the status bar, the sidebar header and the keyboard shortcuts.
use super::{
    ChromeState, EntryAction, EntryPoint, HerdrKind, Place, edit_bar, entry_move::LegacyBands,
    footer, overflow_menu, overflow_menu::OverflowItem, rail, shortcuts,
};

/// Herdr 2 keeps one tab per session (q4) and its split in the Herdr 2 band
/// until L8 moves it.
pub(crate) const HERDR2_TAB_ROW: &str = "Herdr 2 uses its own pane controls for now";

fn point(action: EntryAction, place: Place, enabled: bool) -> EntryPoint {
    EntryPoint {
        action,
        place,
        enabled,
        reason: None,
    }
}

fn overflow_action(item: OverflowItem) -> Option<EntryAction> {
    match item {
        OverflowItem::Paste => Some(EntryAction::Paste),
        OverflowItem::PasteImage => Some(EntryAction::PasteImage),
        OverflowItem::SelectText => Some(EntryAction::Select),
        OverflowItem::Copy => Some(EntryAction::Copy),
        OverflowItem::Find => Some(EntryAction::Find),
        OverflowItem::CommandPalette => Some(EntryAction::Shortcuts),
        OverflowItem::Reconnect => Some(EntryAction::Reconnect),
        OverflowItem::NewTab => Some(EntryAction::NewTab),
        OverflowItem::Split => Some(EntryAction::Split),
        OverflowItem::CloseTab
        | OverflowItem::Detach
        | OverflowItem::Pane1
        | OverflowItem::Pane2
        | OverflowItem::SplitDirection => None,
    }
}

/// The legacy toolbar and bands, while `legacy` still draws them.
fn legacy_entry_points(state: &ChromeState, legacy: LegacyBands, out: &mut Vec<EntryPoint>) {
    let herdr1 = state.kind == HerdrKind::Herdr1;
    if legacy.toolbar {
        out.push(point(EntryAction::Hosts, Place::Toolbar, true));
        out.push(point(EntryAction::Settings, Place::Toolbar, true));
        out.push(point(
            EntryAction::Reconnect,
            Place::Toolbar,
            !state.connected,
        ));
    }
    if legacy.handoff {
        for action in [
            EntryAction::Paste,
            EntryAction::PasteImage,
            EntryAction::Select,
            EntryAction::Copy,
        ] {
            out.push(point(action, Place::HandoffBand, true));
        }
    }
    if legacy.actions && herdr1 {
        for action in [
            EntryAction::NewTab,
            EntryAction::Split,
            EntryAction::Shortcuts,
            EntryAction::ToggleSidebar,
        ] {
            out.push(point(action, Place::ActionsBand, true));
        }
    }
    // Herdr 2's split, pane and Detach band, until L8 retires it.
    if !herdr1 && !super::herdr2_frame::panes::band_retired(state) {
        out.push(point(EntryAction::Split, Place::CoreBand, state.connected));
    }
}

/// The places L2a adds: tab row, ... menu, status bar, sidebar header and the
/// ⌘ shortcuts and long press of the terminal (Herdr 1).
fn frame_entry_points(state: &ChromeState, out: &mut Vec<EntryPoint>) {
    let herdr1 = state.kind == HerdrKind::Herdr1;
    for action in [EntryAction::NewTab, EntryAction::Split] {
        out.push(EntryPoint {
            action,
            place: Place::TabRow,
            enabled: herdr1,
            reason: (!herdr1).then_some(HERDR2_TAB_ROW),
        });
    }
    for entry in overflow_menu::overflow_menu_items(state) {
        if let Some(action) = overflow_action(entry.item) {
            out.push(EntryPoint {
                action,
                place: Place::OverflowMenu,
                enabled: entry.enabled,
                reason: entry.reason,
            });
        }
    }
    if state.offers_reconnect() {
        out.push(point(EntryAction::Reconnect, Place::StatusBar, true));
    }
    out.push(point(EntryAction::Shortcuts, Place::StatusBar, true));
    if state.sidebar_forced {
        // The status bar hides "Show sidebar" while the width forces the
        // rail; the rail's own toggle carries the reason.
    } else if state.sidebar_hidden {
        out.push(point(EntryAction::ToggleSidebar, Place::StatusBar, true));
    } else {
        out.push(point(
            EntryAction::ToggleSidebar,
            Place::SidebarHeader,
            true,
        ));
    }
    if herdr1 {
        for action in [
            EntryAction::Paste,
            EntryAction::Copy,
            EntryAction::NewTab,
            EntryAction::Split,
            EntryAction::Shortcuts,
            EntryAction::ToggleSidebar,
        ] {
            // Command-B refuses with the reason while the width forces the
            // rail (actions.rs, L4 review F1).
            let reason = (action == EntryAction::ToggleSidebar)
                .then(|| rail::toggle_unavailable(state))
                .flatten();
            out.push(EntryPoint {
                reason,
                ..point(action, Place::Keyboard, reason.is_none())
            });
        }
        if state.find_available {
            out.push(point(EntryAction::Find, Place::Keyboard, true));
        }
        for action in [EntryAction::Paste, EntryAction::Copy] {
            out.push(point(action, Place::LongPress, true));
        }
    }
}

/// Every entry point the chrome offers in `state`, legacy bands included
/// while `legacy` still shows them.
pub(crate) fn chrome_entry_points(state: &ChromeState, legacy: LegacyBands) -> Vec<EntryPoint> {
    let mut out = Vec::new();
    legacy_entry_points(state, legacy, &mut out);
    frame_entry_points(state, &mut out);
    out.extend(footer::footer_entry_points(state));
    out.extend(rail::rail_entry_points(state));
    out.extend(edit_bar::edit_bar_entry_points(state));
    out.extend(shortcuts::shortcuts_entry_points(state));
    out
}

/// The required set of plan v2 5.1 rule 2 for `state`.
///
/// New space had no place at the start (plan v2 5.2: "なし") and gets its
/// first one in L2b's sidebar footer, so it becomes required as soon as the
/// footer exists; every other action is required from L2a on.
pub(crate) fn required_entry_actions(state: &ChromeState) -> Vec<EntryAction> {
    let mut required = vec![EntryAction::Hosts, EntryAction::Settings];
    if !footer::footer_items(state.kind).is_empty() {
        required.push(EntryAction::NewSpace);
    }
    required.extend([
        EntryAction::Paste,
        EntryAction::PasteImage,
        EntryAction::Select,
        EntryAction::Copy,
    ]);
    if state.offers_reconnect() {
        required.push(EntryAction::Reconnect);
    }
    if state.find_available {
        required.push(EntryAction::Find);
    }
    required.extend([
        EntryAction::NewTab,
        EntryAction::Split,
        EntryAction::Shortcuts,
        EntryAction::ToggleSidebar,
    ]);
    required
}

#[cfg(test)]
#[path = "entry_points_tests.rs"]
mod tests;
