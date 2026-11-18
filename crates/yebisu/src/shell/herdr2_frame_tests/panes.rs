use super::*;
use crate::shell::overflow_menu::{OverflowEntry, OverflowItem};
use panes::{
    BAND_CONTROLS, EDGE, FOCUSED_EDGE, PaneRect, SIDE_BY_SIDE_LINE, STACKED_LINE,
    band_control_homes, frame_lines, pane_at, pane_rects,
};

fn entry(item: OverflowItem) -> OverflowEntry {
    OverflowEntry {
        item,
        enabled: true,
        reason: None,
    }
}

#[test]
fn herdr2_pane_frames_draw_a_line_between_panes() {
    // One pane: the whole frame, no line.
    let one = pane_rects(80, 24, 1, false);
    assert_eq!(
        one,
        [PaneRect {
            x: 0,
            y: 0,
            cols: 80,
            rows: 24
        }]
    );
    assert!(frame_lines(&one, false, 0).is_empty());
    // Side by side: one column of `│` between the panes, no overlap.
    let side = pane_rects(80, 24, 2, false);
    assert_eq!(
        side[0],
        PaneRect {
            x: 0,
            y: 0,
            cols: 39,
            rows: 24
        }
    );
    assert_eq!(
        side[1],
        PaneRect {
            x: 40,
            y: 0,
            cols: 40,
            rows: 24
        }
    );
    let line = frame_lines(&side, false, 1);
    assert_eq!(line.len(), 24);
    assert!(
        line.iter()
            .all(|c| c.x == 39 && c.symbol == SIDE_BY_SIDE_LINE)
    );
    // The half next to the focused pane takes the accent; the other is plain.
    assert!(
        line.iter()
            .filter(|c| c.y >= 12)
            .all(|c| c.fg == FOCUSED_EDGE)
    );
    assert!(line.iter().filter(|c| c.y < 12).all(|c| c.fg == EDGE));
    // Wire cell colours are tagged RGB (`0x02RRGGBB`), never bare RGB.
    assert!(line.iter().all(|c| c.fg >> 24 == 0x02));
    // Stacked: one row of `─`.
    let stacked = pane_rects(80, 24, 2, true);
    assert_eq!(
        stacked[0],
        PaneRect {
            x: 0,
            y: 0,
            cols: 80,
            rows: 11
        }
    );
    assert_eq!(
        stacked[1],
        PaneRect {
            x: 0,
            y: 12,
            cols: 80,
            rows: 12
        }
    );
    let line = frame_lines(&stacked, true, 0);
    assert_eq!(line.len(), 80);
    assert!(line.iter().all(|c| c.y == 11 && c.symbol == STACKED_LINE));
    // No pane cell is a line cell.
    for cell in frame_lines(&side, false, 0) {
        assert_eq!(pane_at(&side, cell.x, cell.y), None);
    }
    // A tiny frame still gives each pane a cell.
    assert!(
        pane_rects(2, 1, 2, false)
            .iter()
            .all(|r| r.cols >= 1 && r.rows >= 1)
    );
}

#[test]
fn herdr2_pane_tap_focuses_the_pane() {
    let side = pane_rects(80, 24, 2, false);
    assert_eq!(pane_at(&side, 0, 0), Some(0));
    assert_eq!(pane_at(&side, 38, 23), Some(0));
    assert_eq!(pane_at(&side, 39, 5), None, "the line is no pane");
    assert_eq!(pane_at(&side, 40, 5), Some(1));
    assert_eq!(pane_at(&side, 80, 5), None);
    let stacked = pane_rects(80, 24, 2, true);
    assert_eq!(pane_at(&stacked, 10, 3), Some(0));
    assert_eq!(pane_at(&stacked, 10, 20), Some(1));
}

#[test]
fn herdr2_control_band_retires_only_when_every_control_has_a_home() {
    let partial = [entry(OverflowItem::Paste), entry(OverflowItem::Detach)];
    // Detach already lives in the ... menu; Split, panes and direction not.
    let homes = band_control_homes(&partial);
    assert_eq!(homes.len(), BAND_CONTROLS.len());
    assert_eq!(
        homes
            .iter()
            .find(|(l, _)| *l == "Detach")
            .and_then(|(_, h)| *h),
        Some(OverflowItem::Detach)
    );
    assert!(
        homes.iter().any(|(_, h)| h.is_none()),
        "a control without a home keeps the band"
    );
    // The real menu: the band stays exactly while some control has no home.
    let state = herdr2(true);
    let real = band_control_homes(&crate::shell::overflow_menu::overflow_menu_items(&state));
    assert_eq!(
        panes::band_retired(&state),
        real.iter().all(|(_, h)| h.is_some())
    );
    // Every band label is a label the menu can show (OCR keeps one wording).
    for (label, home) in real {
        if let Some(item) = home {
            assert_eq!(item.label(), label);
        }
    }
}
