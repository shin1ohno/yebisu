//! L8: Herdr 2 pane frames and the control band's homes (plan v2 7.14).
//! Owner: L8.
//!
//! Herdr 1's daemon draws the line between panes into the frame cells, so
//! the app paints it like any other cell. herdr-core sends each terminal
//! alone; yebisu composes the panes and draws that line itself, one cell
//! wide, with the half next to the focused pane in the accent colour. The control
//! band (CB) retires only when every control it holds has a home in the
//! frame (plan v2 5.1 rule 1: a move adds the new place first).
use crate::shell::{
    ChromeState, HerdrKind,
    overflow_menu::{OverflowEntry, OverflowItem, overflow_menu_items},
};

/// Band labels; the ... menu reuses them so OCR checks keep one wording.
pub(crate) const SPLIT: &str = "Split (2 panes)";
pub(crate) const PANE_1: &str = "Pane 1";
pub(crate) const PANE_2: &str = "Pane 2";
pub(crate) const ORIENTATION: &str = "Toggle split direction";
pub(crate) const DETACH: &str = "Detach";

/// Every control the Herdr 2 band holds, in band order.
pub(crate) const BAND_CONTROLS: [&str; 5] = [SPLIT, PANE_1, PANE_2, ORIENTATION, DETACH];

/// The separator between two panes: `│` side by side, `─` stacked.
pub(crate) const SIDE_BY_SIDE_LINE: &str = "│";
pub(crate) const STACKED_LINE: &str = "─";
/// The focused pane's edge: the frame's accent (nord8), as a wire cell
/// colour (`CellData::fg`: RGB is `0x02RRGGBB`).
pub(crate) const FOCUSED_EDGE: u32 = 0x0288_c0d0;
/// The other edges (nord3), as a wire cell colour.
pub(crate) const EDGE: u32 = 0x024c_566a;

/// One pane's cells inside the composed frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaneRect {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

/// The panes of a `cols` x `rows` frame. Two panes leave one cell between
/// them for the frame line; each pane keeps at least one cell.
pub(crate) fn pane_rects(cols: u16, rows: u16, count: usize, vertical: bool) -> Vec<PaneRect> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    if count < 2 {
        return vec![PaneRect {
            x: 0,
            y: 0,
            cols,
            rows,
        }];
    }
    let span = if vertical { rows } else { cols };
    let first = (span.saturating_sub(1) / 2).max(1);
    let second = span.saturating_sub(first + 1).max(1);
    let start = first + 1;
    if vertical {
        vec![
            PaneRect {
                x: 0,
                y: 0,
                cols,
                rows: first,
            },
            PaneRect {
                x: 0,
                y: start,
                cols,
                rows: second,
            },
        ]
    } else {
        vec![
            PaneRect {
                x: 0,
                y: 0,
                cols: first,
                rows,
            },
            PaneRect {
                x: start,
                y: 0,
                cols: second,
                rows,
            },
        ]
    }
}

/// One frame-line cell: position, symbol and colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineCell {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) symbol: &'static str,
    pub(crate) fg: u32,
}

/// The line between two panes. The half next to the focused pane takes the
/// focused colour, the rest the plain edge colour.
pub(crate) fn frame_lines(rects: &[PaneRect], vertical: bool, active: usize) -> Vec<LineCell> {
    let [first, second] = rects else {
        return Vec::new();
    };
    if vertical {
        let y = first.y + first.rows;
        let half = first.cols / 2;
        (0..first.cols)
            .map(|x| LineCell {
                x,
                y,
                symbol: STACKED_LINE,
                fg: if (active == 0) == (x < half) {
                    FOCUSED_EDGE
                } else {
                    EDGE
                },
            })
            .filter(|cell| cell.y < second.y)
            .collect()
    } else {
        let x = first.x + first.cols;
        let half = first.rows / 2;
        (0..first.rows)
            .map(|y| LineCell {
                x,
                y,
                symbol: SIDE_BY_SIDE_LINE,
                fg: if (active == 0) == (y < half) {
                    FOCUSED_EDGE
                } else {
                    EDGE
                },
            })
            .filter(|cell| cell.x < second.x)
            .collect()
    }
}

/// The pane under a frame cell (a pane tap), or `None` on the line.
pub(crate) fn pane_at(rects: &[PaneRect], x: u16, y: u16) -> Option<usize> {
    rects.iter().position(|r| {
        x >= r.x && x < r.x.saturating_add(r.cols) && y >= r.y && y < r.y.saturating_add(r.rows)
    })
}

/// Where each band control lives in a ... menu: the entry with the same
/// label, or `None` while it has no home there yet.
pub(crate) fn band_control_homes(
    menu: &[OverflowEntry],
) -> Vec<(&'static str, Option<OverflowItem>)> {
    BAND_CONTROLS
        .iter()
        .map(|label| {
            let home = menu
                .iter()
                .find(|entry| entry.item.label() == *label)
                .map(|entry| entry.item);
            (*label, home)
        })
        .collect()
}

/// The band stays until every control it holds has a home in the Herdr 2
/// ... menu of this chrome state.
pub(crate) fn band_retired(state: &ChromeState) -> bool {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connected: true,
        ..*state
    };
    band_control_homes(&overflow_menu_items(&state))
        .iter()
        .all(|(_, home)| home.is_some())
}
