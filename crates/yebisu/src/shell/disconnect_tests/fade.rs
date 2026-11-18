//! A disconnected region stops, stays dimmed 30 s, then goes to 0 (mod
//! point 7). The clock is injected.
use super::super::*;

#[test]
fn disconnected_region_fades_after_30s() {
    assert_eq!(disconnect_fade(Duration::ZERO), DisconnectFade::Dimmed);
    assert_eq!(
        disconnect_fade(Duration::from_millis(29_999)),
        DisconnectFade::Dimmed
    );
    assert_eq!(
        disconnect_fade(Duration::from_secs(30)),
        DisconnectFade::Cleared
    );
    assert_eq!(
        disconnect_fade(Duration::from_secs(3600)),
        DisconnectFade::Cleared
    );

    let start = Instant::now();
    let at = |secs: u64| start + Duration::from_secs(secs);
    let mut clock = DisconnectClock::default();
    clock.observe(true, at(0));
    assert_eq!(clock.fade(at(0)), DisconnectFade::Visible);
    assert!(clock.fade(at(0)).interactive());
    // The line drops at 5 s: input stops at once, the last screen dims.
    clock.observe(false, at(5));
    let dimmed = clock.fade(at(6));
    assert_eq!(dimmed, DisconnectFade::Dimmed);
    assert!(!dimmed.interactive());
    assert_eq!(dimmed.opacity(), DIMMED_OPACITY);
    assert!(dimmed.keeps_size());
    // Later observations while down do not restart the clock.
    clock.observe(false, at(20));
    assert_eq!(clock.fade(at(34)), DisconnectFade::Dimmed);
    let cleared = clock.fade(at(35));
    assert_eq!(cleared, DisconnectFade::Cleared);
    assert!(!cleared.keeps_size());
    assert_eq!(cleared.opacity(), 0.0);
    // A reconnect shows the region again.
    clock.observe(true, at(40));
    assert_eq!(clock.fade(at(41)), DisconnectFade::Visible);
}
