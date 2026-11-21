//! The state behind the rule: observation, retries and the snapshot reader.
use super::support::{HOST, snapshot};
use crate::layout::PaneMode::{Single, Split};
use crate::shell::single_pane::{
    ColumnMetrics, SinglePaneState, ZoomRequest, focused_tab, herdr1_zoom_request, tab_key,
};
use std::time::{Duration, Instant};

#[test]
fn unanswered_zoom_is_given_up_not_repeated() {
    let start = Instant::now();
    let mut state = SinglePaneState::default();
    state.record_sent(HOST, 1, ZoomRequest::Zoom, 2, start, Default::default());
    state.observe(HOST, 1, false, start + Duration::from_secs(1));
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, Single, 2, false), None);
    // Never shown: recorded as failed, and still not sent again however long
    // the column stays narrow.
    for seconds in [4, 9, 60] {
        state.observe(HOST, 1, false, start + Duration::from_secs(seconds));
        assert_eq!(herdr1_zoom_request(&state, HOST, 1, Single, 2, false), None);
    }
    // A split is a new situation for the tab.
    assert_eq!(
        herdr1_zoom_request(&state, HOST, 1, Single, 3, false),
        Some(ZoomRequest::Zoom)
    );
    // So is the column going wide and narrow again.
    state.column_changed(HOST, 1, false);
    assert_eq!(
        herdr1_zoom_request(&state, HOST, 1, Single, 2, false),
        Some(ZoomRequest::Zoom)
    );
}

#[test]
fn unanswered_release_is_given_up_until_the_column_changes() {
    let start = Instant::now();
    let mut state = SinglePaneState::default();
    state.record_sent(HOST, 1, ZoomRequest::Zoom, 2, start, Default::default());
    state.observe(HOST, 1, true, start);
    state.record_sent(HOST, 1, ZoomRequest::Release, 2, start, Default::default());
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, Split, 2, true), None);
    for seconds in [4, 9, 60] {
        state.observe(HOST, 1, true, start + Duration::from_secs(seconds));
        assert_eq!(herdr1_zoom_request(&state, HOST, 1, Split, 2, true), None);
    }
    // Narrow, then wide again: the release is tried once more.
    state.column_changed(HOST, 1, true);
    assert_eq!(
        herdr1_zoom_request(&state, HOST, 1, Split, 2, true),
        Some(ZoomRequest::Release)
    );
    // The zoom is gone: the record goes with it.
    state.observe(HOST, 1, false, start + Duration::from_secs(70));
    state.observe(HOST, 1, true, start + Duration::from_secs(71));
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, Split, 2, true), None);
}

#[test]
fn closed_tabs_are_forgotten() {
    let now = Instant::now();
    let mut state = SinglePaneState::default();
    state.record_sent(HOST, 1, ZoomRequest::Zoom, 2, now, Default::default());
    state.observe(HOST, 1, true, now);
    state.retain_tabs(HOST, &[2, 3]);
    assert_eq!(herdr1_zoom_request(&state, HOST, 1, Split, 2, true), None);
}

#[test]
fn focused_tab_counts_only_its_own_panes() {
    let mut multi = snapshot(3, true);
    let mut other = multi.tabs[0].clone();
    other.tab_id = "w1:t2".into();
    other.zoomed = false;
    other.focused = false;
    multi.tabs.push(other);
    let mut stray = multi.panes[0].clone();
    stray.pane_id = "w1:p9".into();
    stray.tab_id = "w1:t2".into();
    multi.panes.push(stray);
    let view = focused_tab(&multi).unwrap_or_else(|| panic!("no focused tab"));
    assert_eq!((view.panes, view.zoomed), (3, true));
    assert_eq!(view.key, tab_key("w1:t1"));
    assert_ne!(tab_key("w1:t1"), tab_key("w2:t1"));
    multi.focused_tab_id = None;
    assert_eq!(focused_tab(&multi), None);
}

#[test]
fn minimum_cells_follow_the_cell_size() {
    let metrics = |cell_width, cell_height| ColumnMetrics {
        width: 600.0,
        height: 600.0,
        cell_width,
        cell_height,
    };
    assert_eq!(metrics(10.0, 20.0).minimum_cells(), (28, 11));
    assert_eq!(metrics(8.4, 17.0).minimum_cells(), (34, 13));
    // An unmeasured cell asks for one cell, never zero.
    assert_eq!(metrics(0.0, 0.0).minimum_cells(), (1, 1));
}
