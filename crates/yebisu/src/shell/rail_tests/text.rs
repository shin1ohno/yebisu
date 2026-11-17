use crate::shell::{
    rail::{LABEL_PT, rail_metrics},
    rail_badges::BADGE_TEXT_PT,
};

#[test]
fn rail_text_is_at_least_11pt() {
    // The constants themselves, not only what `rail_metrics` returns.
    assert!(std::hint::black_box(LABEL_PT) >= 11.0);
    assert!(std::hint::black_box(BADGE_TEXT_PT) >= 11.0);
    // At every text scale, including one below 1 or one that is not a number.
    for scale in [0.0, 0.5, 1.0, 1.3, 2.0, f32::NAN, f32::INFINITY, -4.0] {
        let metrics = rail_metrics(scale);
        assert!(metrics.label_pt >= 11.0, "label at scale {scale}");
        assert!(metrics.badge_pt >= 11.0, "badge at scale {scale}");
    }
}
