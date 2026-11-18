use super::estimate;
use crate::shell::rail_badges::*;

#[test]
fn combined_badge_accessibility_name_keeps_both_counts() {
    // The picture shows 123 but the spoken name names both counts.
    let mode = rail_badges(120, 3, &estimate);
    assert_eq!(mode, RailBadges::Combined);
    let texts = badge_texts(120, 3, mode);
    assert_eq!(texts.right.as_deref(), Some("99+"));
    let spoken = badge_accessibility(120, 3).unwrap_or_default();
    assert!(spoken.contains("120 agents waiting"), "{spoken}");
    assert!(spoken.contains("3 agents in error"), "{spoken}");
    assert!(!spoken.contains("123"), "{spoken}");
    // Singular wording, and the same name in the Two mode.
    let spoken = badge_accessibility(1, 1).unwrap_or_default();
    assert_eq!(spoken, "1 agent waiting, 1 agent in error");
}

#[test]
fn rail_shows_error_badge_only_with_errors() {
    for mode in [RailBadges::Two, RailBadges::Combined] {
        let texts = badge_texts(2, 0, mode);
        assert_eq!(texts.left, None, "no Error badge without errors ({mode:?})");
        assert_eq!(texts.right.as_deref(), Some("2"));
    }
    assert_eq!(rail_badges(2, 0, &estimate), RailBadges::Two);
    let only_errors = badge_texts(0, 4, RailBadges::Two);
    assert_eq!(only_errors.left.as_deref(), Some("4"));
    assert_eq!(only_errors.right, None);
    let none = badge_texts(0, 0, RailBadges::Two);
    assert_eq!((none.left, none.right), (None, None));
    assert_eq!(
        badge_accessibility(2, 0).as_deref(),
        Some("2 agents waiting")
    );
    assert_eq!(badge_accessibility(0, 0), None);
    assert_eq!(badge_label(99), "99");
    assert_eq!(badge_label(100), "99+");
}
