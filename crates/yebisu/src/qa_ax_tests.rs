//! S0 probe tests: the QA accessibility layer stays out of other builds and
//! `qa.state` changes exactly when its content does.
use super::*;

fn control(x: f32) -> Control {
    Control {
        id: MORE,
        label: "More",
        frame: [x, 24.0, 44.0, 36.0],
    }
}

#[test]
fn qa_ax_only_in_simulator_with_env() {
    assert!(!allowed(true, |_| false), "no QA variable");
    assert!(!allowed(false, |_| true), "not a Simulator build");
    assert!(allowed(true, |name| name == ENV));
    assert_eq!((MORE, STATE), ("qa.shell.more", "qa.state"));
    assert!(!allowed(true, |name| name == "YEBISU_P12_QA"));
    const { assert!(!BUILD, "host tests are not a Simulator build") };
    assert!(!enabled());
    assert!(mark(MORE, "More", |_| false).is_none());
    assert!(note_press(MORE, false).is_none());
}

#[test]
fn qa_state_seq_moves_only_with_content() {
    let first = next(None, vec![control(10.0)], State::default());
    let Some(first) = first else {
        panic!("first publish missing");
    };
    assert_eq!(first.1.seq, 1);
    assert!(next(Some(&first), vec![control(10.0)], State::default()).is_none());
    // A moved frame republishes the layout without a new state value.
    let Some(moved) = next(Some(&first), vec![control(12.0)], State::default()) else {
        panic!("moved frame not republished");
    };
    assert_eq!(moved.1.seq, 1);
    let pressed = State {
        presses: 1,
        overflow: true,
        ..State::default()
    };
    let Some(pressed) = next(Some(&moved), vec![control(12.0)], pressed) else {
        panic!("press not republished");
    };
    assert_eq!(pressed.1.seq, 2);
    let json: serde_json::Value = match serde_json::from_str(&state_json(&pressed.1)) {
        Ok(value) => value,
        Err(error) => panic!("state is not JSON: {error}"),
    };
    assert_eq!(json["seq"], 2);
    assert_eq!(json["presses"], 1);
    assert_eq!(json["overflow"], true);
    assert!(json["pasteboard"].is_null());
}

#[test]
fn pasteboard_nonce_names_press() {
    assert_eq!(pasteboard_nonce(42, 3), "yebisu-qa-ax-42-3");
}
