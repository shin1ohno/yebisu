use super::super::{Input, PaneMode, TWO_PANE_MIN_WIDTH, pane_mode};
use super::{artboard_10, artboard_11, layout, window};

#[test]
fn narrow_column_forces_single_pane() {
    assert_eq!(TWO_PANE_MIN_WIDTH, 568.0);
    assert_eq!(pane_mode(567.0, 2), PaneMode::Single);
    assert_eq!(pane_mode(568.0, 2), PaneMode::Split);
    assert_eq!(pane_mode(300.0, 3), PaneMode::Single);
    // Through the layout: sidebar 300 + rule 1 leaves 567 / 568.
    let two = |width| {
        layout(Input {
            pane_count: 2,
            ..window(width, 834.0)
        })
        .pane_mode
    };
    assert_eq!(two(868.0), PaneMode::Single);
    assert_eq!(two(869.0), PaneMode::Split);
    assert_eq!(layout(artboard_10()).pane_mode, PaneMode::Single);
    assert_eq!(layout(artboard_11()).pane_mode, PaneMode::Single);
}

#[test]
fn single_pane_tab_is_unaffected() {
    for width in [0.0, 280.0, 567.0, 568.0, 1200.0] {
        assert_eq!(pane_mode(width, 1), PaneMode::Single, "{width}");
        assert_eq!(pane_mode(width, 0), PaneMode::Single, "{width}");
    }
}

#[test]
fn wide_column_keeps_two_panes() {
    for width in [568.0, 700.0, 1075.0] {
        assert_eq!(pane_mode(width, 2), PaneMode::Split, "{width}");
        assert_eq!(pane_mode(width, 4), PaneMode::Split, "{width}");
    }
}
