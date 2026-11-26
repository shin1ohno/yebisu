//! iPadOS 26's window controls: the close, minimize and resize buttons the
//! system draws over one top corner of a window that is not full screen. In
//! a left-to-right system they sit over the sidebar column's header band; in
//! a right-to-left one, over the tab row's pinned buttons (+, split, ...).
//!
//! The platform measures their reach: herdr-uikit's `window_control_insets`
//! is UIKit's safe area with horizontal corner adaptation less the plain safe
//! area, per physical edge, so it says how far top-bar content must start
//! from each side (66 pt in an iPadOS 26.5 window, narrow or wide). It gives
//! no height. The controls fit in the header band: in a window whose safe
//! area starts 10 pt down, they span about 21 to 42 pt from its top
//! (Simulator). The full sidebar pads its header by the reach; the rail, too
//! narrow to hold anything beside the controls, leaves the whole header band
//! empty instead; the tab row pads its trailing end.
//!
//! All zero means no window controls (owner decision 2026-11-20): a
//! full-screen window whose corners report nothing and a window herdr-uikit
//! has not sampled yet both read zero, and the SDK has no flag to tell them
//! apart. The known cost: an iPadOS 26 window that launches
//! windowed can draw its first frames without clearance. The controls may
//! overlap the sidebar header or the tab row's pinned buttons for ~130 ms,
//! until the SDK reports their reach. The sample arrives as a GPUI insets
//! change, which redraws the window with the clearance.
use super::{Insets, SIDEBAR_HEADER_HEIGHT, SIDEBAR_HEADER_PADDING};

/// How far one top corner's reach must pass the other's to be the controls
/// rather than the corner allowance UIKit reports on both top corners of
/// every window. Measured UIKit values (iPadOS 26.5 Simulator, iPad Pro
/// 13-inch (M5), the app's `window_controls` records): with the controls the
/// two corners are 56.5 to 57 pt apart (left 66 / right 9.5 in a narrow and
/// a wide window, left 9.0 / right 66 right-to-left); without them at most
/// 0.5 pt (9.5 / 9.0 full screen; herdr-uikit's own record 5.5 / 5.5). The
/// 12 pt threshold is 11.5 pt above the largest measured gap without
/// controls and 44.5 pt below the smallest one with controls. A hypothetical
/// left 52 / right 64 pt sample is outside these measurements. Its 12 pt
/// gap gets no clearance (see `layout/tests/controls.rs`); QA records the raw
/// edges so new platform behavior can be recognized.
/// Comparing the two corners of one window, not a corner with a fixed size,
/// keeps a device with a larger allowance on both corners from gaining a
/// band.
const CONTROLS_MIN_EXCESS: f32 = 12.0;

/// The window controls' reach past the safe area on each physical side, in
/// points, as herdr-uikit last sampled it for the window; all zero is a
/// window without controls (see the module comment).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlsReach {
    pub left: f32,
    pub right: f32,
}

/// Where the window keeps clear of the window controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlsClearance {
    /// Padding before the full sidebar's header content: the header's own
    /// padding, or the controls' reach while they show on the left.
    pub header_leading: f32,
    /// Height the rail keeps empty above its list while the controls show on
    /// the left: the full sidebar's header height, else 0.
    pub rail_top: f32,
    /// Padding after the tab row's pinned buttons while the controls show on
    /// the right: their reach, else 0.
    pub tab_row_trailing: f32,
}

impl Default for ControlsClearance {
    fn default() -> Self {
        Self {
            header_leading: SIDEBAR_HEADER_PADDING,
            rail_top: 0.0,
            tab_row_trailing: 0.0,
        }
    }
}

impl ControlsClearance {
    /// Padding before the tab row's first item, for a tab row starting at
    /// `tab_row_x` in a window whose sidebar column starts at `sidebar_x`:
    /// on a Mac whose traffic lights sit over the content's top row
    /// (herdr-uikit `traffic_lights_over_content`) with them on the left, how
    /// far the row's start falls short of the reach the full sidebar's header
    /// keeps (`header_leading`). The full sidebar ends past the traffic lights,
    /// so its tab row gets none; the 72 pt rail, or no sidebar at all, does
    /// not. 0 on an iPad: iPadOS 26's window controls keep today's layout.
    pub fn tab_row_leading(
        self,
        traffic_lights_over_content: bool,
        sidebar_x: f32,
        tab_row_x: f32,
    ) -> f32 {
        if !traffic_lights_over_content || self.rail_top <= 0.0 {
            return 0.0;
        }
        (sidebar_x + self.header_leading - tab_row_x).max(0.0)
    }
}

impl ControlsReach {
    /// The clearance the shell chrome keeps for this reach (`clearance`),
    /// for a surface drawn over the shell, such as the Settings sheet.
    pub fn clearance(self) -> ControlsClearance {
        clearance(&Insets {
            window_controls: self,
            ..Insets::default()
        })
    }
}

/// The clearance for the controls' reach in `insets`: nothing for all zero
/// (no controls), nor for a corner allowance alone. The status bar's icons
/// (`Insets::status_icons`) sit on the other side: past the tab row's pinned
/// buttons with the controls on the left, before the sidebar header's
/// content with them on the right.
pub(super) fn clearance(insets: &Insets) -> ControlsClearance {
    // `max` also maps a NaN reach to 0.
    let left = insets.window_controls.left.max(0.0);
    let right = insets.window_controls.right.max(0.0);
    let icons = insets.status_icons.max(0.0);
    if left - right > CONTROLS_MIN_EXCESS {
        ControlsClearance {
            header_leading: left,
            rail_top: SIDEBAR_HEADER_HEIGHT,
            tab_row_trailing: icons,
        }
    } else if right - left > CONTROLS_MIN_EXCESS {
        ControlsClearance {
            header_leading: SIDEBAR_HEADER_PADDING.max(icons),
            rail_top: if icons > 0.0 {
                SIDEBAR_HEADER_HEIGHT
            } else {
                0.0
            },
            tab_row_trailing: right,
        }
    } else {
        ControlsClearance::default()
    }
}

/// The width the status bar's icons take at the top-right of a window that
/// reaches the screen's top edge, in points, with a gap before the tab row's
/// last button: 113 pt measured in the owner's external-display screenshot
/// (2026-11-26, location, VPN, Wi-Fi and battery at 1.44 px per point) plus
/// the 8 pt inline gap. UIKit reports no layout region for them: the
/// corner-adapted safe area gives only the window's 9-9.5 pt corner allowance
/// on that side. [Unverified] Other icon sets are wider or narrower.
pub const STATUS_ICONS_REACH: f32 = 113.0 + crate::shell::style::metric::INLINE_GAP;

/// The smallest top safe area that holds the status bar: 24 pt in full
/// screen and 32 pt in the owner's external-display window, against 10 pt
/// for a window clear of the screen's top edge (iPadOS 26.5 Simulator).
pub const STATUS_BAR_MIN_TOP: f32 = 20.0;

/// Where the top row goes in a window with `reported_top` as its top safe
/// area. In an iPad window with window controls (`windowed`), the row moves
/// up into that band, beside the controls, as on a Mac. If the band holds
/// the status bar, the row keeps clear of its icons. Everywhere else,
/// including full screen and a Mac, the safe area stays as UIKit reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopBand {
    pub safe_top: f32,
    pub status_icons: f32,
}

pub fn top_band(reported_top: f32, windowed: bool) -> TopBand {
    let reported_top = reported_top.max(0.0);
    if !windowed {
        return TopBand {
            safe_top: reported_top,
            status_icons: 0.0,
        };
    }
    TopBand {
        safe_top: 0.0,
        status_icons: if reported_top >= STATUS_BAR_MIN_TOP {
            STATUS_ICONS_REACH
        } else {
            0.0
        },
    }
}

impl ControlsClearance {
    /// Whether this clears window controls at all: anything but the default
    /// a window without controls (or with a corner allowance alone) gets.
    pub fn shows_controls(self) -> bool {
        self != Self::default()
    }
}
