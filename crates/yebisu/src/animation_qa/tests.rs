use super::{Event, STUCK_AFTER, Watch};
use std::time::{Duration, Instant};

fn after(start: Instant, seconds: u64) -> Instant {
    start + Duration::from_secs(seconds)
}

#[test]
fn records_once_when_animations_outlast_the_threshold() {
    let start = Instant::now();
    let mut watch = Watch::new();
    assert_eq!(watch.observe(start, true), Event::Quiet);
    assert_eq!(
        watch.observe(after(start, STUCK_AFTER.as_secs() - 1), true),
        Event::Quiet
    );
    assert_eq!(
        watch.observe(after(start, STUCK_AFTER.as_secs()), true),
        Event::Stuck
    );
    assert_eq!(
        watch.observe(after(start, STUCK_AFTER.as_secs() + 1), true),
        Event::Quiet
    );
    assert_eq!(watch.observe(after(start, 60), true), Event::Quiet);
}

#[test]
fn short_animations_are_never_recorded() {
    let start = Instant::now();
    let mut watch = Watch::new();
    for second in 0..60 {
        // Each animation lasts 2 s, the usual keyboard transition.
        let running = second % 5 < 2;
        assert_eq!(watch.observe(after(start, second), running), Event::Quiet);
    }
}

#[test]
fn a_gap_without_animations_starts_a_new_stretch() {
    let start = Instant::now();
    let mut watch = Watch::new();
    assert_eq!(watch.observe(start, true), Event::Quiet);
    assert_eq!(watch.observe(after(start, 10), true), Event::Stuck);
    assert_eq!(
        watch.observe(after(start, 11), false),
        Event::Ended(Duration::from_secs(11))
    );
    assert_eq!(watch.observe(after(start, 12), true), Event::Quiet);
    assert_eq!(watch.observe(after(start, 21), true), Event::Quiet);
    assert_eq!(watch.observe(after(start, 22), true), Event::Stuck);
}

#[test]
fn a_process_records_a_bounded_number_of_stretches() {
    let start = Instant::now();
    let mut watch = Watch::new();
    let mut recorded = 0;
    for stretch in 0..20u64 {
        let base = stretch * 100;
        // The gap ends the previous stretch (reported as Ended when it was recorded).
        assert_ne!(watch.observe(after(start, base), false), Event::Stuck);
        for second in 0..=STUCK_AFTER.as_secs() + 1 {
            recorded +=
                usize::from(watch.observe(after(start, base + 1 + second), true) == Event::Stuck);
        }
    }
    assert_eq!(recorded, 8);
}

#[test]
fn a_recorded_stretch_reports_how_long_it_lasted_once() {
    let start = Instant::now();
    let mut watch = Watch::new();
    assert_eq!(watch.observe(start, true), Event::Quiet);
    assert_eq!(watch.observe(after(start, 10), true), Event::Stuck);
    assert_eq!(watch.observe(after(start, 70), true), Event::Quiet);
    assert_eq!(
        watch.observe(after(start, 71), false),
        Event::Ended(Duration::from_secs(71))
    );
    assert_eq!(watch.observe(after(start, 72), false), Event::Quiet);
    // A short stretch that was never recorded ends quietly.
    assert_eq!(watch.observe(after(start, 80), true), Event::Quiet);
    assert_eq!(watch.observe(after(start, 82), false), Event::Quiet);
}
