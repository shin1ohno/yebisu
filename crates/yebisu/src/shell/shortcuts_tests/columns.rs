//! 3 columns at 760 pt, 1 column below, never wider than the window.
use super::super::*;

#[test]
fn narrow_popover_single_column() {
    assert_eq!(popover_columns(760.), 3);
    assert_eq!(popover_columns(1366.), 3);
    assert_eq!(popover_columns(759.5), 1);
    assert_eq!(popover_columns(507.), 1);
    assert_eq!(popover_columns(320.), 1);
    // The popover is 760 pt wide and stays inside a narrow window.
    assert_eq!(popover_width(1366.), POPOVER_WIDTH);
    for width in [320., 507., 694., 759.] {
        assert!(popover_width(width) <= width - 32.);
    }
    assert_eq!(ROW_HEIGHT, 36.);
    // The columns follow the popover's own width (design text D1): a window
    // 780 pt wide gives a 748 pt popover, which has 1 column.
    assert_eq!(popover_columns(popover_width(780.)), 1);
    assert_eq!(popover_columns(popover_width(792.)), 3);
}
