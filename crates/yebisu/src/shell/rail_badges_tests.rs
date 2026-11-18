//! L4 badge rules, one topic per file. Names follow plan v2 7.9.

#[path = "rail_badges_tests/fit.rs"]
mod fit;
#[path = "rail_badges_tests/labels.rs"]
mod labels;

/// Bold 11 pt digits are about 6.5 pt wide (plan v2 7.9 estimate); the
/// real widths are measured on the Simulator by the rail itself.
pub(super) fn estimate(label: &str) -> f32 {
    label.chars().count() as f32 * 6.5
}

/// A measure that reports one exact width for every label.
pub(super) fn fixed(width: f32) -> impl Fn(&str) -> f32 {
    move |_| width
}
