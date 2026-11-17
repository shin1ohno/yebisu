use crate::shell::rail::{ITEM_BASE_HEIGHT, rail_metrics};
use crate::shell::rail_badges::BADGE_HEIGHT;

#[test]
fn row_height_follows_text_scale() {
    // The rail version of S-11: buttons and badges grow with the text.
    let base = rail_metrics(1.0);
    assert_eq!(base.item_height, ITEM_BASE_HEIGHT);
    assert_eq!(base.badge_height, BADGE_HEIGHT);
    let mut last = base;
    for scale in [1.1, 1.3, 1.6, 2.0, 3.0] {
        let metrics = rail_metrics(scale);
        assert!(metrics.item_height > last.item_height, "scale {scale}");
        assert!(metrics.badge_height >= last.badge_height, "scale {scale}");
        assert!(metrics.label_pt > last.label_pt, "scale {scale}");
        last = metrics;
    }
    assert_eq!(rail_metrics(2.0).item_height, 2.0 * ITEM_BASE_HEIGHT);
    // A smaller scale never shrinks a button below the touch size.
    assert_eq!(rail_metrics(0.5).item_height, ITEM_BASE_HEIGHT);
    assert!(std::hint::black_box(ITEM_BASE_HEIGHT) >= 44.0);
}
