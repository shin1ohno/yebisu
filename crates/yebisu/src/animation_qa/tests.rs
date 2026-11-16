use super::{STUCK_AFTER, Watch};
use std::time::{Duration, Instant};

fn after(start: Instant, seconds: u64) -> Instant {
    start + Duration::from_secs(seconds)
}

#[test]
fn records_once_when_animations_outlast_the_threshold() {
    let start = Instant::now();
    let mut watch = Watch::new();
    assert!(!watch.observe(start, true));
    assert!(!watch.observe(after(start, STUCK_AFTER.as_secs() - 1), true));
    assert!(watch.observe(after(start, STUCK_AFTER.as_secs()), true));
    assert!(!watch.observe(after(start, STUCK_AFTER.as_secs() + 1), true));
    assert!(!watch.observe(after(start, 60), true));
}

#[test]
fn short_animations_are_never_recorded() {
    let start = Instant::now();
    let mut watch = Watch::new();
    for second in 0..60 {
        // Each animation lasts 2 s, the usual keyboard transition.
        let running = second % 5 < 2;
        assert!(!watch.observe(after(start, second), running));
    }
}

#[test]
fn a_gap_without_animations_starts_a_new_stretch() {
    let start = Instant::now();
    let mut watch = Watch::new();
    assert!(!watch.observe(start, true));
    assert!(watch.observe(after(start, 10), true));
    assert!(!watch.observe(after(start, 11), false));
    assert!(!watch.observe(after(start, 12), true));
    assert!(!watch.observe(after(start, 21), true));
    assert!(watch.observe(after(start, 22), true));
}

#[test]
fn a_process_records_a_bounded_number_of_stretches() {
    let start = Instant::now();
    let mut watch = Watch::new();
    let mut recorded = 0;
    for stretch in 0..20u64 {
        let base = stretch * 100;
        assert!(!watch.observe(after(start, base), false));
        for second in 0..=STUCK_AFTER.as_secs() + 1 {
            recorded += usize::from(watch.observe(after(start, base + 1 + second), true));
        }
    }
    assert_eq!(recorded, 8);
}
