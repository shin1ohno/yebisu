//! Horizontal allocation: a width shortage takes M2 first and collapses the
//! sidebar to the rail last (Q2).
use super::{M2_RANGE, ModSlot, RAIL_WIDTH, RULE, Region, SIDEBAR_WIDTH, TERMINAL_MIN_WIDTH};

pub(super) struct Columns {
    pub sidebar: f32,
    pub collapsed: bool,
    /// Terminal column width (the find bar, M4, the terminal, M3, QA strip).
    pub column: f32,
    /// M2 width, 0 when absent or withdrawn.
    pub dock: f32,
    /// Even the rail leaves less than one pane's minimum width.
    pub too_narrow: bool,
}

fn fits_width(column: f32) -> bool {
    column >= TERMINAL_MIN_WIDTH
}

/// M2's width beside `beside` of room: it gives up width down to its minimum
/// before it is withdrawn.
fn dock_beside(beside: f32, m2: ModSlot, withdrawn: &mut Vec<Region>) -> f32 {
    let mut dock = match m2 {
        ModSlot::Wants(width) => M2_RANGE.clamp(width),
        ModSlot::Empty | ModSlot::VersionMismatch => 0.0,
    };
    if dock > 0.0 && !fits_width(beside - dock) {
        let shrunk = beside - TERMINAL_MIN_WIDTH;
        if shrunk >= M2_RANGE.min {
            dock = shrunk;
        } else {
            dock = 0.0;
            withdrawn.push(Region::M2);
        }
    }
    dock
}

pub(super) fn allocate(
    usable_width: f32,
    m2: ModSlot,
    user_rail: bool,
    withdrawn: &mut Vec<Region>,
) -> Columns {
    if user_rail {
        // The user chose 72: the rail at any width, M2 beside it when it fits.
        let beside = (usable_width - RAIL_WIDTH - RULE).max(0.0);
        let dock = dock_beside(beside, m2, withdrawn);
        let column = beside - dock;
        return Columns {
            sidebar: RAIL_WIDTH.min(usable_width),
            collapsed: true,
            column,
            dock,
            too_narrow: !fits_width(column),
        };
    }
    let beside_sidebar = usable_width - SIDEBAR_WIDTH - RULE;
    let dock = dock_beside(beside_sidebar, m2, withdrawn);
    if fits_width(beside_sidebar - dock) {
        return Columns {
            sidebar: SIDEBAR_WIDTH,
            collapsed: false,
            column: beside_sidebar - dock,
            dock,
            too_narrow: false,
        };
    }
    // Reached only with no dock: M2 was absent or has just been withdrawn.
    let column = (usable_width - RAIL_WIDTH - RULE).max(0.0);
    Columns {
        sidebar: RAIL_WIDTH.min(usable_width),
        collapsed: true,
        column,
        dock: 0.0,
        too_narrow: !fits_width(column),
    }
}
