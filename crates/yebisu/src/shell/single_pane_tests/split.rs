//! Rule: splitting while the column is narrow keeps one pane on screen.
use super::support::{HOST, snapshot};
use crate::shell::single_pane::{SinglePaneState, ZoomRequest, focused_tab, herdr1_zoom_request};
use std::time::Instant;

/// One frame of the rule against `snapshot`, as `enforce_herdr1` runs it.
fn frame(
    state: &mut SinglePaneState,
    width: f32,
    panes: usize,
    zoomed: bool,
    now: Instant,
) -> Option<ZoomRequest> {
    let view = focused_tab(&snapshot(panes, zoomed)).unwrap_or_else(|| panic!("no focused tab"));
    state.observe(HOST, view.key, view.zoomed, now);
    let request = herdr1_zoom_request(state, HOST, view.key, width, view.panes, view.zoomed);
    if let Some(request) = request {
        state.record_sent(HOST, view.key, request, view.panes, now);
    }
    request
}

#[test]
fn split_while_narrow_keeps_single_pane() {
    let now = Instant::now();
    let mut state = SinglePaneState::default();
    // One pane in a narrow column: nothing to do.
    assert_eq!(frame(&mut state, 400.0, 1, false, now), None);
    // The user splits: two panes, not zoomed -> one Zoom, not repeated.
    assert_eq!(
        frame(&mut state, 400.0, 2, false, now),
        Some(ZoomRequest::Zoom)
    );
    assert_eq!(frame(&mut state, 400.0, 2, false, now), None);
    // Split again before the host showed the zoom: a new situation, so the
    // new pane is zoomed too.
    assert_eq!(
        frame(&mut state, 400.0, 3, false, now),
        Some(ZoomRequest::Zoom)
    );
    // The host shows the zoom and keeps it through another split.
    assert_eq!(frame(&mut state, 400.0, 3, true, now), None);
    assert_eq!(frame(&mut state, 400.0, 4, true, now), None);
    // The host drops the zoom when the next pane appears: zoom once more.
    assert_eq!(
        frame(&mut state, 400.0, 5, false, now),
        Some(ZoomRequest::Zoom)
    );
    assert_eq!(frame(&mut state, 400.0, 5, false, now), None);
}

#[test]
fn split_in_a_wide_column_stays_two_panes() {
    let now = Instant::now();
    let mut state = SinglePaneState::default();
    assert_eq!(frame(&mut state, 700.0, 2, false, now), None);
    assert_eq!(frame(&mut state, 700.0, 3, false, now), None);
}
