//! Rule: a terminal is never sent a size under 280 x 206 pt, also after the
//! column is divided between panes.
use super::support::{column, size};
use crate::core::screen::pane_size_for;

#[test]
fn per_pane_size_never_below_minimum() -> crate::Result<()> {
    let mut checked = 0;
    for cell in [(10.0_f32, 20.0_f32), (8.4, 17.0), (12.5, 25.0)] {
        for width in (280..=1400).step_by(37) {
            for height in (206..=1100).step_by(41) {
                let metrics = column(width as f32, height as f32, cell);
                let (least_cols, least_rows) = metrics.minimum_cells();
                let cols = (width as f32 / cell.0).floor() as u16;
                let rows = (height as f32 / cell.1).floor() as u16;
                if cols < least_cols || rows < least_rows {
                    continue;
                }
                for vertical in [false, true] {
                    let sent = pane_size_for(size(cols, rows)?, 2, vertical, Some(metrics))?;
                    assert!(
                        sent.cols() >= least_cols && sent.rows() >= least_rows,
                        "{width}x{height} cell {cell:?} vertical {vertical}: {}x{}",
                        sent.cols(),
                        sent.rows()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 500, "{checked}");

    // During a resize the last composed frame can still be smaller than the
    // newly measured column. Both the physical budget and its actual cells
    // must accommodate two panes.
    let retained = size(90, 20)?;
    let wide = Some(column(1000., 1000., (10., 20.)));
    assert_eq!(pane_size_for(retained, 2, true, wide)?, retained);
    Ok(())
}

#[test]
fn division_that_would_undercut_the_minimum_gives_the_whole_column() -> crate::Result<()> {
    // 600 x 300 pt stacked: two panes would be 150 pt tall each.
    let metrics = Some(column(600.0, 300.0, (10.0, 20.0)));
    let whole = size(60, 15)?;
    assert_eq!(pane_size_for(whole, 2, true, metrics)?, whole);
    // The same test at 600 pt wide: the two bordered panes and their padding
    // cannot hold 28 text columns each. Jumping between the unframed and
    // framed resize budgets must not make that decision oscillate.
    let metrics = Some(column(600.0, 800.0, (10.0, 20.0)));
    assert_eq!(
        pane_size_for(size(57, 38)?, 2, false, metrics)?,
        size(57, 38)?
    );
    assert_eq!(
        pane_size_for(size(51, 36)?, 2, false, metrics)?,
        size(51, 36)?
    );
    let metrics = Some(column(700.0, 800.0, (10.0, 20.0)));
    let sent = pane_size_for(size(61, 36)?, 2, false, metrics)?;
    assert_eq!((sent.cols(), sent.rows()), (30, 36));
    Ok(())
}
