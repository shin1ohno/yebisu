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
    // Two 34-column panes and the line between them (L8, plan v2 7.14).
    assert_eq!(cells.iter().filter(|s| *s == "A").count(), 340);
    assert_eq!(cells.iter().filter(|s| *s == "B").count(), 340);
    assert_eq!(cells.iter().filter(|s| *s == "│").count(), 10);
    assert!(live.status.contains("Side by side"), "{}", live.status);
    assert_eq!(pane_size_for(size, 2, false, wide)?.cols(), 34);
    // Before the first measurement nothing changes.
    let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), None);
    assert_eq!(symbols(&live), cells);
    assert_eq!(pane_size_for(size, 2, false, None)?.cols(), 34);
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
    // 71 columns can hold two 280 pt text grids and one divider cell.
    // The mapped window also needs room for the native pane frames.
    let size = size(71, 10)?;
    let screens = [screen("A", 35, 10), screen("B", 71, 10)];
    for width in [560.0, 563.0, 567.0] {
        let narrow = Some(column(width, 600.0, (8.0, 20.0)));
        assert_eq!(pane_size_for(size, 2, false, narrow)?, size, "{width}");
        let live = publish_for(&screens, &saved(1, false)?, size, (1, 1, 0), narrow);
        assert!(symbols(&live).iter().all(|s| s == "B"), "{width}");
    }
    // The 8 pt inner padding (Q3 B, 2026-11-26) takes 16 pt less for two
    // panes than the design's 12 pt: they fit from 636 pt, not 652.
    for width in [568., 635.] {
        let tight = Some(column(width, 600.0, (8.0, 20.0)));
        assert_eq!(pane_size_for(size, 2, false, tight)?, size, "{width}");
    }
    let wide = Some(column(636., 600.0, (8.0, 20.0)));
    assert_eq!(pane_size_for(size, 2, false, wide)?.cols(), 35);
    Ok(())
}

#[test]
fn herdr2_size_sent_matches_the_rect_drawn() -> crate::Result<()> {
    // Review 9c55 S1: the size each terminal is sent is the rect it is
    // drawn in, and the line cell is counted once by both.
    let wide = Some(column(800.0, 600.0, (10.0, 20.0)));
    let size = size(80, 10)?;
    let sent = pane_size_for(size, 2, false, wide)?;
    assert_eq!((sent.cols(), sent.rows()), (39, 10));
    let screens = [screen("A", 39, 10), screen("B", 39, 10)];
    let live = publish_for(&screens, &saved(0, false)?, size, (1, 1, 0), wide);
    let cells = symbols(&live);
    assert_eq!(cells.iter().filter(|s| *s == "A").count(), 390);
    assert_eq!(cells.iter().filter(|s| *s == "B").count(), 390);
    assert_eq!(cells.iter().filter(|s| *s == "│").count(), 10);
    // One pane drawn (narrow column): no line, the pane over the whole frame.
    let narrow = Some(column(400.0, 600.0, (10.0, 20.0)));
    let one = [screen("A", 80, 10), screen("B", 80, 10)];
    let live = publish_for(&one, &saved(0, false)?, size, (1, 1, 0), narrow);
    assert!(symbols(&live).iter().all(|s| s == "A"));
    // A frame too small for two panes and their line is one pane, in any
    // column, and nothing is drawn outside it.
    let tiny = super::support::size(2, 1)?;
    assert_eq!(pane_size_for(tiny, 2, false, wide)?, tiny);
    let live = publish_for(
        &[screen("A", 2, 1), screen("B", 2, 1)],
        &saved(1, false)?,
        tiny,
        (1, 1, 0),
        None,
    );
    assert_eq!(symbols(&live).len(), 2);
    assert!(symbols(&live).iter().all(|s| s == "B"));
    Ok(())
}
