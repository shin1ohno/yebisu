//! Too-small window: the message replaces the terminal and nothing is sent
//! (Q1, D2).
use super::super::*;
use crate::{
    layout::{self, Insets, TERMINAL_MIN_HEIGHT, TERMINAL_MIN_WIDTH},
    live::Live,
    mock_peer,
    terminal_input::TerminalInput,
};
use sheprd_client::{ClientEvent, protocol::ClientShellSnapshot};
use std::sync::Arc;

fn layout_for(window: Size) -> layout::Layout {
    layout::compute(&layout::Input {
        window,
        insets: Insets::default(),
        pane_count: 1,
        ..layout::Input::default()
    })
}

fn ready_input() -> TerminalInput {
    let mut live = Live::default();
    live.connected = true;
    let snapshot: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    let mut input = TerminalInput::default();
    input.sync(&live, 1, None);
    input
}

/// Presses `keys` one by one and queues what the input generates.
fn type_keys(input: &mut TerminalInput, keys: &[&'static str]) -> usize {
    let mut queued = 0;
    for name in keys {
        if let Some(generated) = input.commit(name)
            && input.queue(generated)
        {
            queued += 1;
        }
    }
    queued
}

#[test]
fn too_small_window_message_and_no_send() {
    // L0's verdict decides: a window that cannot hold 280×206 is too small,
    // a full iPad window is not.
    let small = too_small(layout_for(Size::new(320.0, 300.0)).terminal)
        .unwrap_or_else(|| panic!("320×300 must be too small"));
    assert_eq!(
        small.minimum,
        Size::new(TERMINAL_MIN_WIDTH, TERMINAL_MIN_HEIGHT)
    );
    assert!(too_small(layout_for(Size::new(1376.0, 1032.0)).terminal).is_none());
    assert!(too_small(layout_for(Size::new(605.0, 834.0)).terminal).is_none());
    // Every window size L0 calls too small is reported, never a rect below
    // the minimum or at or below 0.
    for width in (200..=700).step_by(20) {
        for height in (150..=500).step_by(10) {
            let fit = layout_for(Size::new(width as f32, height as f32)).terminal;
            match (fit.rect(), too_small(fit)) {
                (Some(rect), None) => {
                    assert!(rect.width >= TERMINAL_MIN_WIDTH && rect.height >= TERMINAL_MIN_HEIGHT);
                }
                (None, Some(_)) => {}
                other => panic!("{width}×{height}: {other:?}"),
            }
        }
    }
    // A Fits rect under the minimum (or 0) is still reported, not sent.
    let zero = TerminalFit::Fits(Rect::new(0.0, 0.0, 0.0, 0.0));
    assert!(too_small(zero).is_some());

    // The message says what is needed and what is left.
    let message = too_small_message(&small);
    assert!(message.contains("280 × 206"), "{message}");
    assert!(
        message.contains(&format!("{:.0} ×", small.available.width)),
        "{message}"
    );
    assert_eq!(TOO_SMALL_TITLE, "Window too small");

    // No input reaches the terminal while the window is too small.
    let mut input = ready_input();
    assert_eq!(type_keys(&mut input, &["a", "b"]), 2);
    input.hold_for_size(true);
    assert!(
        input.next().is_none(),
        "queued input left a too-small window"
    );
    assert_eq!(
        type_keys(&mut input, &["c", "d"]),
        0,
        "input queued while too small"
    );
    assert!(input.next().is_none());
    // Growing the window releases new input; what was typed while too small
    // was never queued.
    let mut input = ready_input();
    input.hold_for_size(true);
    input.hold_for_size(false);
    assert_eq!(type_keys(&mut input, &["e"]), 1);
    assert!(
        input.next().is_some(),
        "input stays blocked after the window grew"
    );
}

#[test]
fn hold_survives_detach() {
    let mut input = ready_input();
    input.hold_for_size(true);
    input.detach();
    let mut live = Live::default();
    live.connected = true;
    let snapshot: ClientShellSnapshot =
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture: {e}"));
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    input.sync(&live, 2, None);
    assert_eq!(type_keys(&mut input, &["x"]), 0);
    assert!(input.next().is_none());
}
