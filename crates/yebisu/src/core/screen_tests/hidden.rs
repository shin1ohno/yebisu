//! Rule: a pane hidden by the one-pane rule keeps the size it last had
//! (plan v2 6.3). `integrator-patches/02-core-worker-hidden-size.patch`
//! makes the worker use this.
use super::support::{column, size};
use crate::core::screen::pane_sizes;

#[test]
fn hidden_pane_is_not_resized() -> crate::Result<()> {
    let whole = size(40, 10)?;
    let narrow = Some(column(400.0, 600.0, (10.0, 20.0)));
    assert_eq!(
        pane_sizes(whole, 2, false, 1, narrow)?,
        [None, Some(whole)],
        "only the drawn pane is sent the column"
    );
    assert_eq!(pane_sizes(whole, 2, false, 0, narrow)?, [Some(whole), None]);
    let wide = Some(column(700.0, 600.0, (10.0, 20.0)));
    // 70 columns: two panes of 34 and the line between them (L8, 7.14).
    let halves = size(34, 10)?;
    assert_eq!(
        pane_sizes(size(70, 10)?, 2, false, 1, wide)?,
        [Some(halves), Some(halves)]
    );
    assert_eq!(
        pane_sizes(size(70, 10)?, 2, false, 1, None)?,
        [Some(halves), Some(halves)]
    );
    Ok(())
}
