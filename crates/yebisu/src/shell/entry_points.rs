//! L2a: the entry-point model behind `chrome_entry_points_cover_required_set`.
//! Owner: L2a. Other streams contribute through their own functions:
//! L2b `footer::footer_entry_points`, L4 `rail::rail_entry_points`,
//! L5 `edit_bar::edit_bar_entry_points`, L6 `shortcuts::shortcuts_entry_points`.
use super::{ChromeState, EntryAction, EntryPoint, entry_move::LegacyBands};

/// Every entry point the chrome offers in `state`, legacy bands included
/// while `legacy` still shows them.
pub(crate) fn chrome_entry_points(_state: &ChromeState, _legacy: LegacyBands) -> Vec<EntryPoint> {
    Vec::new()
}

/// The required set of plan v2 5.1 rule 2 for `state`.
pub(crate) fn required_entry_actions(_state: &ChromeState) -> Vec<EntryAction> {
    Vec::new()
}
