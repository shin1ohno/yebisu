//! Settings › Sidebar: how much room the sidebar's rows take (design
//! fidelity plan, Wave 0a). The canvas moves the old toolbar's
//! "Board: Dense / Roomy" there (canvas.json map note), and the owner chose
//! the canvas over the toggle's removal in q7 (decision 2026-11-20). Stored with
//! the other two-way choices (`input_options::FIELDS`) as
//! `"sidebar_density": "roomy"`; Dense, the default, is not written.
use super::input_options::Field;

/// Sidebar row density. Dense draws the artboards' rows (SidebarSpec: host
/// 64, space 56, AGENTS 44 pt, `shell::sidebar`). Roomy gives every row more
/// vertical room, as the old board's Roomy did; stream A decides how much
/// and draws both, and stream E puts the choice in Settings › Sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SidebarDensity {
    #[default]
    Dense,
    Roomy,
}

/// The stored choice: `dense` is the default and is not written.
pub(crate) const SIDEBAR_DENSITY: Field = Field {
    key: "sidebar_density",
    off: "dense",
    on: "roomy",
    default: false,
    get: |values| values.sidebar_density == SidebarDensity::Roomy,
    set: |values, roomy| {
        values.sidebar_density = if roomy {
            SidebarDensity::Roomy
        } else {
            SidebarDensity::Dense
        }
    },
};

#[cfg(test)]
#[path = "sidebar_density_tests.rs"]
mod tests;
