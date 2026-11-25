//! The terminal cursor's last drawn bounds, so the SDK hears when it moves
//! (#139).
//!
//! herdr-uikit shows the system's input mode indicator at the caret UIKit
//! reads from `caretRectForPosition:`, which is the terminal cursor cell
//! (`InputGeometry::cursor`, the rectangle `bounds_for_range` returns). A
//! terminal's selection stays at 0..0 while output moves the cursor, so
//! nothing else tells the SDK; `Window::invalidate_character_coordinates`
//! makes GPUI report the new caret (`update_ime_position`). Comparing the
//! drawn bounds covers a cell move, a pane, tab or space switch, a font size
//! change and a resize. A same-cell repaint reports nothing.

use gpui::{Bounds, Pixels};

#[derive(Debug, Default)]
pub(crate) struct CursorWatch {
    last: Option<Bounds<Pixels>>,
    invalidations: u64,
}

impl CursorWatch {
    /// Records this frame's cursor and returns whether it differs from the
    /// last frame's.
    pub(crate) fn moved(&mut self, cursor: Bounds<Pixels>) -> bool {
        if self.last == Some(cursor) {
            return false;
        }
        self.last = Some(cursor);
        true
    }

    /// Counts one report to GPUI; the caller invalidates in the same place.
    pub(crate) fn reported(&mut self) {
        self.invalidations = self.invalidations.saturating_add(1);
    }

    /// How often the cursor's move was reported to GPUI.
    #[cfg_attr(
        not(any(test, target_abi = "sim")),
        allow(dead_code, reason = "read by the Simulator's qa.state and by tests")
    )]
    pub(crate) fn invalidations(&self) -> u64 {
        self.invalidations
    }
}

#[cfg(test)]
#[path = "input_indicator_tests.rs"]
mod tests;
