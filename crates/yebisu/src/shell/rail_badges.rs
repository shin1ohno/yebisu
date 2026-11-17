//! L4: the rail's two badges (q3). Owner: L4.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RailBadges {
    Two,
    Combined,
}

/// `measure` returns the rendered width of a badge label in points.
pub(crate) fn rail_badges(
    _needs: usize,
    _error: usize,
    _measure: &dyn Fn(&str) -> f32,
) -> RailBadges {
    RailBadges::Two
}
