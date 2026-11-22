//! L8: Herdr 2 pane frames and the control band's homes (plan v2 7.14).
//! Owner: L8.
//!
//! Herdr 1's daemon draws the line between panes into the frame cells, so
//! the app paints it like any other cell. herdr-core sends each terminal
//! alone; yebisu composes the panes and draws that line itself, one cell
//! wide, with the half next to the focused pane in the accent colour. The control
//! band (CB) retires only when every control it holds has a home in the
//! frame (plan v2 5.1 rule 1: a move adds the new place first).
use crate::shell::style::color;
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
pub(crate) const FOCUSED_EDGE: u32 = 0x0200_0000 | color::ACCENT;
/// The other edges (nord3), as a wire cell colour.
pub(crate) const EDGE: u32 = 0x0200_0000 | color::SLATE;

/// One pane's cells inside the composed frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaneRect {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

/// The panes of a `cols` x `rows` frame. `count` panes share the span with
/// one line cell between neighbours; every pane gets the same size, which is
/// both the size sent to its terminal and the rect drawn, and every rect
/// stays inside the frame (the cells left over after the last pane stay
/// blank). A span with less than one cell per pane plus its lines (below 3
/// for two panes) falls back to one pane over the whole frame.
pub(crate) fn pane_rects(cols: u16, rows: u16, count: usize, vertical: bool) -> Vec<PaneRect> {
    let whole = PaneRect {
        x: 0,
        y: 0,
        cols: cols.max(1),
        rows: rows.max(1),
    };
    let span = usize::from(if vertical { rows } else { cols });
    let count = count.max(1);
    if count < 2 || span < 2 * count - 1 {
        return vec![whole];
    }
    let each = (span - (count - 1)) / count;
    let each = u16::try_from(each).unwrap_or(u16::MAX);
    (0..count)
        .map(|index| {
            let offset = u16::try_from(index * (usize::from(each) + 1)).unwrap_or(u16::MAX);
            if vertical {
                PaneRect {
                    x: 0,
                    y: offset,
                    cols: whole.cols,
                    rows: each,
                }
            } else {
                PaneRect {
                    x: offset,
                    y: 0,
                    cols: each,
                    rows: whole.rows,
                }
            }
        })
        .collect()
}

/// One frame-line cell: position, symbol and colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineCell {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) symbol: &'static str,
    pub(crate) fg: u32,
}

/// The lines between neighbouring panes. On each line, the half next to the
/// focused pane takes the focused colour, the rest the plain edge colour.
/// One pane has no line.
pub(crate) fn frame_lines(rects: &[PaneRect], vertical: bool, active: usize) -> Vec<LineCell> {
    let mut cells = Vec::new();
    for (index, pair) in rects.windows(2).enumerate() {
        let [before, after] = pair else {
            continue;
        };
        // The line's half towards the focused pane: `before` is `index`.
        let towards_before = active <= index;
        let touches = active == index || active == index + 1;
        if vertical {
            let y = before.y + before.rows;
            let half = before.cols / 2;
            cells.extend((0..before.cols).filter(|_| y < after.y).map(|x| LineCell {
                x,
                y,
                symbol: STACKED_LINE,
                fg: if touches && towards_before == (x < half) {
                    FOCUSED_EDGE
                } else {
                    EDGE
                },
            }));
        } else {
            let x = before.x + before.cols;
            let half = before.rows / 2;
            cells.extend((0..before.rows).filter(|_| x < after.x).map(|y| LineCell {
                x,
                y,
                symbol: SIDE_BY_SIDE_LINE,
                fg: if touches && towards_before == (y < half) {
                    FOCUSED_EDGE
                } else {
                    EDGE
                },
            }));
        }
    }
    cells
}

/// The pane under a frame cell (a pane tap), or `None` on the line.
pub(crate) fn pane_at(rects: &[PaneRect], x: u16, y: u16) -> Option<usize> {
    rects.iter().position(|r| {
        x >= r.x && x < r.x.saturating_add(r.cols) && y >= r.y && y < r.y.saturating_add(r.rows)
    })
}

/// Where each band control lives in a ... menu: the enabled entry with the
/// same label, or `None` while it has no enabled home there. A disabled
/// entry is not a home: the control could not be used from it.
pub(crate) fn band_control_homes(
    menu: &[OverflowEntry],
) -> Vec<(&'static str, Option<OverflowItem>)> {
    BAND_CONTROLS
        .iter()
        .map(|label| {
            let home = menu
                .iter()
                .find(|entry| entry.enabled && entry.item.label() == *label)
                .map(|entry| entry.item);
            (*label, home)
        })
        .collect()
}

/// Whether `menu` gives every band control an enabled home.
pub(crate) fn band_retired_in(menu: &[OverflowEntry]) -> bool {
    band_control_homes(menu)
        .iter()
        .all(|(_, home)| home.is_some())
}

/// The band stays until every control it holds has an enabled home in the
/// Herdr 2 ... menu. Homes are judged while connected: offline the menu
/// disables its items, and the band does not come back just for that.
pub(crate) fn band_retired(state: &ChromeState) -> bool {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connected: true,
        ..*state
    };
    band_retired_in(&overflow_menu_items(&state))
}
