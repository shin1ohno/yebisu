//! Rules: widening releases the zoom yebisu set, never the user's.
use super::support::{HOST, connected, measure, ready, show_zoomed, zoom_requests};
use crate::shell::single_pane::{SinglePaneState, ZoomRequest, herdr1_zoom_request};
use gpui::TestAppContext;
use std::time::Instant;

#[test]
fn release_needs_a_zoom_yebisu_set() {
    let now = Instant::now();
    let mut state = SinglePaneState::default();
    // Wide and zoomed with no record: the user's zoom.
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, 700.0, 2, true), None);
    state.record_sent(HOST, 1, ZoomRequest::Zoom, 2, now);
    // Sent but not yet shown: nothing to release yet.
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, 700.0, 2, true), None);
    state.observe(HOST, 1, true, now);
    assert_eq!(
        herdr1_zoom_request(&state, HOST, 1, 700.0, 2, true),
        Some(ZoomRequest::Release)
    );
    // Another tab of the same host, and the same tab of another host.
    assert_eq!(herdr1_zoom_request(&state, HOST, 2, 700.0, 2, true), None);
    assert_eq!(
        herdr1_zoom_request(&state, "other", 1, 700.0, 2, true),
        None
    );
}

#[gpui::test]
fn herdr1_forced_zoom_released_on_widen(cx: &mut TestAppContext) {
    let peer = connected(cx);
    // Narrow: yebisu zooms the tab, then the host shows the zoom.
    peer.view.update(cx, |this, _| {
        measure(this, 400.0);
        this.enforce_herdr1(400.0);
    });
    ready(&peer.view, cx);
    assert_eq!(zoom_requests(&peer).len(), 1);
    peer.view.update(cx, |this, _| {
        show_zoomed(this, true);
        this.enforce_herdr1(400.0);
    });
    assert!(
        zoom_requests(&peer).is_empty(),
        "a zoomed tab is left alone"
    );
    // Wide again: exactly one Release, and none while it is in flight.
    peer.view.update(cx, |this, _| {
        measure(this, 700.0);
        this.enforce_herdr1(700.0);
        this.enforce_herdr1(700.0);
    });
    ready(&peer.view, cx);
    peer.view.update(cx, |this, _| this.enforce_herdr1(700.0));
    let sent = zoom_requests(&peer);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["params"]["mode"], "toggle");
}

#[gpui::test]
fn herdr1_user_zoom_not_released(cx: &mut TestAppContext) {
    let peer = connected(cx);
    peer.view.update(cx, |this, _| {
        show_zoomed(this, true);
        for width in [700.0, 400.0, 700.0, 1100.0] {
            measure(this, width);
            this.enforce_herdr1(width);
        }
    });
    ready(&peer.view, cx);
    assert!(zoom_requests(&peer).is_empty());
}
