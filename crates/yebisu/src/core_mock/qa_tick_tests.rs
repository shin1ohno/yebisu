//! T-IM-S4 (#139): the Simulator QA tick of the core mock.
use super::QaTicks;
use std::time::Duration;

fn type_keys(ticks: &mut QaTicks, line: &[u8]) {
    for byte in line {
        ticks.typed(1, std::slice::from_ref(byte));
    }
}

#[test]
fn typed_line_ticks_on_its_channel() {
    let mut ticks = QaTicks::new(true, Duration::ZERO);
    type_keys(&mut ticks, b"yebisu-139-tick\r");
    assert_eq!(ticks.tick(), Some(1));
}

#[test]
fn newline_ends_the_line_too() {
    let mut ticks = QaTicks::new(true, Duration::ZERO);
    ticks.typed(2, b"\nyebisu-139-tick\n");
    assert_eq!(ticks.tick(), Some(2));
}

#[test]
fn first_tick_waits_an_interval() {
    let mut ticks = QaTicks::new(true, Duration::from_secs(3600));
    ticks.typed(1, QaTicks::LINE);
    assert_eq!(ticks.tick(), None);
}

#[test]
fn ticks_need_the_qa_switch() {
    let mut ticks = QaTicks::new(false, Duration::ZERO);
    ticks.typed(1, QaTicks::LINE);
    assert_eq!(
        ticks.tick(),
        None,
        "without YEBISU_CORE_QA the line only echoes"
    );
}

#[test]
fn other_lines_do_not_tick() {
    let mut ticks = QaTicks::new(true, Duration::ZERO);
    type_keys(&mut ticks, b"yebisu-139-tock\r");
    type_keys(&mut ticks, b"yebisu-139-tick");
    assert_eq!(ticks.tick(), None);
}

#[test]
fn ticks_stop_after_sixty() {
    let mut ticks = QaTicks::new(true, Duration::ZERO);
    ticks.typed(3, QaTicks::LINE);
    let channels: Vec<_> = std::iter::from_fn(|| ticks.tick()).take(100).collect();
    assert_eq!(channels, vec![3; 60]);
}
