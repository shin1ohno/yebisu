use super::*;
use crate::shell::overflow_menu::{OverflowEntry, OverflowItem};
use panes::{
    BAND_CONTROLS, EDGE, FOCUSED_EDGE, PaneRect, SIDE_BY_SIDE_LINE, STACKED_LINE,
    band_control_homes, band_retired_in, frame_lines, pane_at, pane_rects,
};

fn entry(item: OverflowItem, enabled: bool) -> OverflowEntry {
    OverflowEntry {
        item,
        enabled,
        reason: (!enabled).then_some("Not connected"),
    }
}

fn rect(x: u16, y: u16, cols: u16, rows: u16) -> PaneRect {
    PaneRect { x, y, cols, rows }
}

#[test]
fn herdr2_pane_frames_draw_a_line_between_panes() {
    // One pane: the whole frame, no line.
    let one = pane_rects(80, 24, 1, false);
    assert_eq!(one, [rect(0, 0, 80, 24)]);
    assert!(frame_lines(&one, false, 0).is_empty());
    // Side by side: equal panes, one column of `│` between them; the size
    // sent (each rect) and the rect drawn are the same, the 80th column
    // stays blank.
    let side = pane_rects(80, 24, 2, false);
    assert_eq!(side, [rect(0, 0, 39, 24), rect(40, 0, 39, 24)]);
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
    assert_eq!(stacked, [rect(0, 0, 80, 11), rect(0, 12, 80, 11)]);
    let line = frame_lines(&stacked, true, 0);
    assert_eq!(line.len(), 80);
    assert!(line.iter().all(|c| c.y == 11 && c.symbol == STACKED_LINE));
    // No pane cell is a line cell.
    for cell in frame_lines(&side, false, 0) {
        assert_eq!(pane_at(&side, cell.x, cell.y), None);
    }
}

#[test]
fn herdr2_pane_rects_stay_inside_the_frame() {
    // Review 9c55 S1: two panes in a 2 x 1 frame fall back to one pane
    // instead of a second rect at x = 2 outside the frame.
    assert_eq!(pane_rects(2, 1, 2, false), [rect(0, 0, 2, 1)]);
    assert_eq!(pane_rects(1, 2, 2, true), [rect(0, 0, 1, 2)]);
    // A span of 3 is the smallest that holds two panes and their line.
    assert_eq!(
        pane_rects(3, 1, 2, false),
        [rect(0, 0, 1, 1), rect(2, 0, 1, 1)]
    );
    assert_eq!(
        pane_rects(5, 3, 2, true),
        [rect(0, 0, 5, 1), rect(0, 2, 5, 1)]
    );
    assert_eq!(pane_rects(0, 0, 2, false), [rect(0, 0, 1, 1)]);
    // Every size up to 12 x 12 and up to three panes: each rect is inside
    // the frame, all panes have the same size, lines never leave the frame
    // and never land on a pane.
    for cols in 0..=12u16 {
        for rows in 0..=12u16 {
            for count in 1..=3 {
                for vertical in [false, true] {
                    let rects = pane_rects(cols, rows, count, vertical);
                    let (fc, fr) = (cols.max(1), rows.max(1));
                    for r in &rects {
                        assert!(r.cols >= 1 && r.rows >= 1, "{cols}x{rows} {count} {r:?}");
                        assert!(
                            r.x + r.cols <= fc && r.y + r.rows <= fr,
                            "{cols}x{rows} {count} {r:?}"
                        );
                        assert_eq!((r.cols, r.rows), (rects[0].cols, rects[0].rows));
                    }
                    assert!(rects.len() == count || rects.len() == 1);
                    for active in 0..rects.len() {
                        for c in frame_lines(&rects, vertical, active) {
                            assert!(c.x < fc && c.y < fr, "{cols}x{rows} {count} {c:?}");
                            assert_eq!(pane_at(&rects, c.x, c.y), None);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn herdr2_pane_tap_focuses_the_pane() {
    let side = pane_rects(80, 24, 2, false);
    assert_eq!(pane_at(&side, 0, 0), Some(0));
    assert_eq!(pane_at(&side, 38, 23), Some(0));
    assert_eq!(pane_at(&side, 39, 5), None, "the line is no pane");
    assert_eq!(pane_at(&side, 40, 5), Some(1));
    assert_eq!(pane_at(&side, 79, 5), None, "the blank column is no pane");
    let stacked = pane_rects(80, 24, 2, true);
    assert_eq!(pane_at(&stacked, 10, 3), Some(0));
    assert_eq!(pane_at(&stacked, 10, 20), Some(1));
}

#[test]
fn herdr2_control_band_retires_only_when_every_control_has_a_home() {
    // Detach is the only band control the ... menu holds on this branch:
    // an enabled Detach is its home, a disabled one is not.
    let homes = band_control_homes(&[entry(OverflowItem::Detach, true)]);
    assert_eq!(homes.len(), BAND_CONTROLS.len());
    assert_eq!(
        homes
            .iter()
            .find(|(l, _)| *l == "Detach")
            .and_then(|(_, h)| *h),
        Some(OverflowItem::Detach)
    );
    let disabled = band_control_homes(&[entry(OverflowItem::Detach, false)]);
    assert!(disabled.iter().all(|(_, h)| h.is_none()));
    // Controls without a home keep the band; so does an empty menu.
    assert!(!band_retired_in(&[entry(OverflowItem::Detach, true)]));
    assert!(!band_retired_in(&[]));
    // The real menu: retired exactly when it carries every control enabled
    // (patch 07 adds them; overflow_menu_tests asserts true and false).
    let state = herdr2(true);
    let real = crate::shell::overflow_menu::overflow_menu_items(&state);
    let has_all = BAND_CONTROLS
        .iter()
        .all(|l| real.iter().any(|e| e.enabled && e.item.label() == *l));
    assert_eq!(panes::band_retired(&state), has_all);
    // Offline the menu disables its items; homes are judged connected.
    assert_eq!(panes::band_retired(&herdr2(false)), has_all);
}
