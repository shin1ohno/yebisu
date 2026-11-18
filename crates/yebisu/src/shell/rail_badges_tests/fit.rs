use super::{estimate, fixed};
use crate::shell::rail_badges::*;

fn spans_for(needs: usize, error: usize, measure: &dyn Fn(&str) -> f32) -> BadgeSpans {
    let mode = rail_badges(needs, error, measure);
    badge_spans(&badge_texts(needs, error, mode), measure)
}

#[test]
fn rail_two_badges_fit_64() {
    // The plan's numbers: three 16 pt parts at x 8..24, 26..38 and 40..56.
    let spans = spans_for(3, 2, &fixed(16.0));
    assert_eq!(spans.left, Some((8.0, 24.0)));
    assert_eq!(spans.right, Some((40.0, 56.0)));
    assert_eq!(dot_span(), (26.0, 38.0));

    // Every pair up to two digits keeps both badges and a 2 pt gap to the dot.
    let (dot_left, dot_right) = dot_span();
    for needs in 1..=99 {
        for error in 1..=99 {
            assert_eq!(
                rail_badges(needs, error, &estimate),
                RailBadges::Two,
                "needs {needs}, error {error}"
            );
            let spans = spans_for(needs, error, &estimate);
            let (left_start, left_end) = spans.left.unwrap_or((f32::NAN, f32::NAN));
            let (right_start, right_end) = spans.right.unwrap_or((f32::NAN, f32::NAN));
            assert!(left_start >= EDGE_INSET && right_end <= BUTTON_WIDTH - EDGE_INSET);
            assert!(left_end + MIN_GAP <= dot_left, "left badge touches the dot");
            assert!(
                dot_right + MIN_GAP <= right_start,
                "right badge touches the dot"
            );
        }
    }
}

#[test]
fn rail_badges_fall_back_to_combined_when_too_wide() {
    // 99+ is three characters, 19.5 pt: wider than the 16 pt a side has.
    assert_eq!(rail_badges(120, 3, &estimate), RailBadges::Combined);
    assert_eq!(rail_badges(3, 120, &estimate), RailBadges::Combined);
    // The boundary: exactly 16 pt still fits (gap 2), a hair more does not.
    assert_eq!(rail_badges(1, 1, &fixed(16.0)), RailBadges::Two);
    assert_eq!(rail_badges(1, 1, &fixed(16.01)), RailBadges::Combined);
    // One side too wide is enough, on either side.
    let wide_needs = |label: &str| if label == "7" { 17.0 } else { 10.0 };
    assert_eq!(rail_badges(7, 1, &wide_needs), RailBadges::Combined);
    let wide_error = |label: &str| if label == "7" { 17.0 } else { 10.0 };
    assert_eq!(rail_badges(1, 7, &wide_error), RailBadges::Combined);
    // The combined badge is one number at the top right, sum of both.
    let texts = badge_texts(120, 3, RailBadges::Combined);
    assert_eq!(texts.left, None);
    assert_eq!(texts.right.as_deref(), Some("99+"));
    let texts = badge_texts(4, 3, RailBadges::Combined);
    assert_eq!(texts.right.as_deref(), Some("7"));
    let spans = badge_spans(&texts, &estimate);
    assert_eq!(spans.left, None);
    assert_eq!(spans.right, Some((40.0, 56.0)));
}
