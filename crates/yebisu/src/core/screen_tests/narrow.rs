//! Rule: below 568 pt the active pane alone is drawn over the whole column.
use super::support::{column, saved, screen, size};
use crate::core::screen::{pane_size_for, publish_for};

fn symbols(live: &crate::live::Live) -> Vec<String> {
    live.surface
        .as_ref()
        .map(|surface| {
            surface
                .frame
                .cells
                .iter()
                .map(|cell| cell.symbol.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn herdr2_narrow_column_draws_active_pane_only() -> crate::Result<()> {
    let size = size(40, 10)?;
    // The hidden pane keeps its old half-size screen; the active one has
    // already been resized to the whole column.
    let screens = [screen("A", 20, 10), screen("B", 40, 10)];
    let narrow = Some(column(400.0, 600.0, (10.0, 20.0)));
    let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), narrow);
    let cells = symbols(&live);
    assert_eq!(cells.len(), 400);
    assert!(cells.iter().all(|s| s == "B"), "only the active pane draws");
    assert!(live.status.contains("One pane"), "{}", live.status);
    // The cursor stays where the active pane puts it, unshifted.
    let cursor = live
        .surface
        .as_ref()
        .and_then(|surface| surface.frame.cursor.clone())
        .map(|cursor| (cursor.x, cursor.y));
    assert_eq!(cursor, Some((0, 0)));
    // The active pane is sized to the column, not to half of it.
    assert_eq!(pane_size_for(size, 2, false, narrow)?, size);
    assert_eq!(pane_size_for(size, 2, true, narrow)?, size);
    // Switching the active pane draws the other one.
    let live = publish_for(&screens, &saved(0, false)?, size, (1, 2, 1), narrow);
    let cells = symbols(&live);
    assert_eq!(cells.iter().filter(|s| *s == "A").count(), 200);
    assert!(cells.iter().all(|s| s != "B"));
    Ok(())
}

#[test]
fn wide_column_keeps_both_panes_drawn() -> crate::Result<()> {
    let size = size(70, 10)?;
    let screens = [screen("A", 35, 10), screen("B", 35, 10)];
    let wide = Some(column(700.0, 600.0, (10.0, 20.0)));
    let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), wide);
    let cells = symbols(&live);
    assert_eq!(cells.iter().filter(|s| *s == "A").count(), 350);
    assert_eq!(cells.iter().filter(|s| *s == "B").count(), 350);
    assert!(live.status.contains("Side by side"), "{}", live.status);
    assert_eq!(pane_size_for(size, 2, false, wide)?.cols(), 35);
    // Before the first measurement nothing changes.
    let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), None);
    assert_eq!(symbols(&live), cells);
    assert_eq!(pane_size_for(size, 2, false, None)?.cols(), 35);
    Ok(())
}

#[test]
fn one_pane_tab_is_drawn_the_same_in_any_column() -> crate::Result<()> {
    let size = size(40, 10)?;
    let one = [screen("A", 40, 10)];
    let mut only = saved(0, false)?;
    only.ids.truncate(1);
    let narrow = Some(column(400.0, 600.0, (10.0, 20.0)));
    let wide = Some(column(700.0, 600.0, (10.0, 20.0)));
    assert_eq!(
        symbols(&publish_for(&one, &only, size, (1, 1, 0), narrow)),
        symbols(&publish_for(&one, &only, size, (1, 1, 0), wide))
    );
    Ok(())
}

#[test]
fn the_band_just_under_568_pt_is_one_pane() -> crate::Result<()> {
    // 8 pt cells: 70 columns fit 560 pt and halve to 35 = 280 pt, so only the
    // 568 pt line (not the cell minimum) makes this one pane.
    let size = size(70, 10)?;
    let screens = [screen("A", 35, 10), screen("B", 70, 10)];
    for width in [560.0, 563.0, 567.0] {
        let narrow = Some(column(width, 600.0, (8.0, 20.0)));
        assert_eq!(pane_size_for(size, 2, false, narrow)?, size, "{width}");
        let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), narrow);
        assert!(symbols(&live).iter().all(|s| s == "B"), "{width}");
    }
    let wide = Some(column(568.0, 600.0, (8.0, 20.0)));
    assert_eq!(pane_size_for(size, 2, false, wide)?.cols(), 35);
    Ok(())
}
