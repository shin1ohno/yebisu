//! iPadOS 26's window controls: the close, minimize and resize buttons the
//! system draws over a window's top-left corner when the window is not full
//! screen. They sit in the band of the sidebar's header, over the sidebar
//! column; the tab row starts right of that column, past their reach.
//!
//! The platform measures their reach: herdr-uikit's `window_control_insets`
//! is UIKit's safe area with horizontal corner adaptation less the plain safe
//! area, so it says how far top-bar content must start from the left edge
//! (66 pt in an iPadOS 26.5 window, narrow or wide). It gives no height. The
//! controls fit in the header band: in a window whose safe area starts 10 pt
//! down, they span about 21 to 42 pt from its top (Simulator). The full
//! sidebar pads its header by the reach; the rail, too narrow to hold
//! anything beside the controls, leaves the whole header band empty instead.
use super::{Insets, SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING};

/// Where the sidebar keeps clear of the window controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlsClearance {
    /// Padding before the full sidebar's header content: the header's own
    /// padding, or the controls' reach when that is wider.
    pub header_leading: f32,
    /// Height the rail keeps empty above its list for the controls: the full
    /// sidebar's header height while they show, else 0.
    pub rail_top: f32,
}

impl Default for ControlsClearance {
    fn default() -> Self {
        Self {
            header_leading: SIDEBAR_HEADER_PADDING,
            rail_top: 0.0,
        }
    }
}

/// The clearance for the controls' reach in `insets`. UIKit also reports a
/// corner allowance without controls (5.5 to 9.5 pt on iPadOS 26.5, full
/// screen included); the header's own padding already covers it, so a reach
/// up to that padding changes nothing and a full-screen window gains no gap.
pub(super) fn clearance(insets: &Insets) -> ControlsClearance {
    // `max` also maps a NaN reach to 0.
    let reach = insets.window_controls.max(0.0);
    if reach <= SIDEBAR_HEADER_PADDING {
        return ControlsClearance::default();
    }
    ControlsClearance {
        header_leading: reach,
        rail_top: SIDEBAR_HEADER_HEIGHT,
    }
}
