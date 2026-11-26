//! Herdr 1 split budget: the daemon's border cells are not left empty.
use super::*;
use crate::{mock_peer, terminal};

const CELL_WIDTH: f32 = 8.4;
const CELL_HEIGHT: f32 = 20.;

/// About the owner's iPad split of 2026-11-25: a 1240 pt wide pane area (its
/// 1048 pt height puts the row count on a rounding edge, so 1046).
fn ipad(width: f32) -> InputGeometry {
    InputGeometry::from_canvas(
        Bounds::new(
            point(px(301.), px(69.)),
            size(
                px(width + 2. * OUTER_SIDE),
                px(1046. + OUTER_TOP + OUTER_SIDE),
            ),
        ),
        CELL_WIDTH,
        CELL_HEIGHT,
    )
}

/// Two panes as Herdr 1 lays them out with its defaults (`mock_peer::split`).
fn herdr1_split(cols: u16, rows: u16, alternate: [bool; 2], stacked: bool) -> PaneSurfaceFrame {
    let mut surface = mock_peer::surface(1);
    surface.frame.width = cols;
    surface.frame.height = rows;
    surface.panes = mock_peer::split::panes(cols, rows, alternate, stacked);
    surface
}

/// The size a frame of `cols` × `rows` makes yebisu send next.
fn next_size(geometry: &InputGeometry, surface: &PaneSurfaceFrame) -> (u16, u16) {
    let viewport = geometry.viewport_size(&geometry.framed_panes(surface));
    let size = terminal::viewport(
        f32::from(viewport.width),
        f32::from(viewport.height),
        CELL_WIDTH,
        CELL_HEIGHT,
    );
    (size.cols, size.rows)
}

/// Send sizes and take the daemon's frames until the size stops changing.
fn settle(geometry: &InputGeometry, alternate: [bool; 2], stacked: bool) -> PaneSurfaceFrame {
    try_settle(geometry, alternate, stacked)
        .unwrap_or_else(|size| panic!("the size did not settle: last {size:?}"))
}

/// `settle`, or the last size sent when it keeps changing.
fn try_settle(
    geometry: &InputGeometry,
    alternate: [bool; 2],
    stacked: bool,
) -> Result<PaneSurfaceFrame, (u16, u16)> {
    settle_from(geometry, (100, 40), alternate, stacked)
}

/// `try_settle` from the frame `start`: a window that shrinks starts above
/// the size it settles at.
fn settle_from(
    geometry: &InputGeometry,
    start: (u16, u16),
    alternate: [bool; 2],
    stacked: bool,
) -> Result<PaneSurfaceFrame, (u16, u16)> {
    let mut size = start;
    for _ in 0..20 {
        let surface = herdr1_split(size.0, size.1, alternate, stacked);
        let next = next_size(geometry, &surface);
        if next == size {
            return Ok(surface);
        }
        size = next;
    }
    Err(size)
}

/// Room the painted columns (the scrollbar column included) and rows leave
/// free inside the padding.
fn slack(placement: &PanePlacement<'_>) -> (f32, f32) {
    let pane = placement.pane;
    let columns = pane.inner_rect.width + u16::from(!pane.alternate_screen_active);
    (
        f32::from(placement.room.size.width) - f32::from(columns) * CELL_WIDTH,
        f32::from(placement.room.size.height) - f32::from(pane.inner_rect.height) * CELL_HEIGHT,
    )
}

/// What the 8 pt inner padding (owner's choice 2026-11-26, Q3 B) frees in
/// each axis against the design's 12 / 14 pt, which the daemon cannot fill:
/// its frame is capped by the pane area, so the body can be up to this much
/// taller or wider than whole cells reach.
const RELEASED: f32 = 8.;

/// Each body ends within a cell of its last column and row, plus what the
/// thinner padding released. The axis the split divides keeps one rounding
/// cell, so there it is within two. The free part is split evenly before
/// the first and after the last cell (`centred`).
fn assert_filled(geometry: &InputGeometry, surface: &PaneSurfaceFrame, stacked: bool) {
    let panes = geometry.framed_panes(surface);
    assert_eq!(panes.len(), 2, "two pane outlines");
    let cells = |split: bool| if split { 2. } else { 1. };
    let (wide, tall) = (
        CELL_WIDTH * cells(!stacked) + RELEASED,
        CELL_HEIGHT * cells(stacked) + RELEASED,
    );
    for placement in &panes {
        let (x, y) = slack(placement);
        let at = (surface.frame.width, surface.frame.height);
        // Within float error: at some widths the outline starts at 9.00006.
        let padding = placement.room.origin - placement.outline.origin;
        assert!(
            (f32::from(padding.x) - (INNER_SIDE + metric::LINE)).abs() < 0.01
                && (f32::from(padding.y) - (INNER_TOP + metric::LINE)).abs() < 0.01,
            "{at:?} {}: {padding:?} of padding inside the border",
            placement.pane.pane_id
        );
        let before = placement.body.origin - placement.room.origin;
        let after = placement.room.bottom_right() - placement.body.bottom_right();
        let even = |a: Pixels, b: Pixels, free: f32| {
            (f32::from(a) - free / 2.).abs() < 0.01 && (f32::from(b) - free / 2.).abs() < 0.01
        };
        assert!(
            even(before.x, after.x, x) && even(before.y, after.y, y),
            "{at:?} {}: {before:?} before the first cell, {after:?} after the last, ({x}, {y}) free",
            placement.pane.pane_id
        );
        assert!(
            (0. ..wide).contains(&x),
            "{at:?} {}: {x} pt after the last column",
            placement.pane.pane_id
        );
        assert!(
            (0. ..tall).contains(&y),
            "{at:?} {}: {y} pt after the last row",
            placement.pane.pane_id
        );
    }
}

#[test]
fn herdr1_split_columns_reach_the_right_edge_of_each_body() {
    // As in the owner's screenshot: a shell on the primary screen beside
    // Claude Code on the alternate screen.
    let geometry = ipad(1240.);
    let surface = settle(&geometry, [false, true], false);
    assert_filled(&geometry, &surface, false);
    // 139 columns left 2 border cells and a rounding cell empty per pane;
    // the 8 pt inner padding (Q3 B) gives two more than the design's 143.
    assert_eq!(surface.frame.width, 145);
    assert_eq!(surface.frame.height, 52);
}

#[test]
fn herdr1_stacked_split_rows_reach_the_bottom_of_each_body() {
    let geometry = ipad(1240.);
    let surface = settle(&geometry, [false, false], true);
    assert_filled(&geometry, &surface, true);
}

/// Every width in 1 pt steps settles, from below and from above, and fills
/// its bodies. Before #199 side-by-side panes alternated between two sizes at
/// 44 of these widths and alternate-screen mixes (142 and 143 columns at
/// 1220 pt).
#[test]
fn herdr1_split_fills_its_bodies_across_widths() {
    for width in 700_u16..=1600 {
        let geometry = ipad(f32::from(width));
        for alternate in [[false, false], [true, true], [false, true], [true, false]] {
            for stacked in [false, true] {
                for start in [(100, 40), (300, 80)] {
                    match settle_from(&geometry, start, alternate, stacked) {
                        Ok(surface) => assert_filled(&geometry, &surface, stacked),
                        Err(size) => panic!(
                            "{width} pt {alternate:?} stacked {stacked} from {start:?}: \
                             did not settle, last {size:?}"
                        ),
                    }
                }
            }
        }
    }
}

/// #199 at 1220 pt: 143 columns split 72 / 71, and the narrow pane's share
/// under half made the budget offer 142, whose even split offered 143 again.
/// The 143-column frame fits, so it holds.
#[test]
fn herdr1_split_holds_an_odd_frame_that_fits() {
    let geometry = ipad(1220.);
    let odd = herdr1_split(143, 52, [false, false], false);
    assert_filled(&geometry, &odd, false);
    assert_eq!(next_size(&geometry, &odd), (143, 52));
    let even = herdr1_split(142, 52, [false, false], false);
    assert_eq!(next_size(&geometry, &even), (143, 52));
}

/// A frame whose cells do not fit their bodies is not held: the budget
/// shrinks it to one that fits.
#[test]
fn herdr1_split_shrinks_a_frame_that_does_not_fit() {
    let geometry = ipad(1220.);
    for (cols, rows) in [(145, 52), (160, 52), (143, 60)] {
        let surface = herdr1_split(cols, rows, [false, true], false);
        let next = next_size(&geometry, &surface);
        assert!(
            next.0 <= cols && next.1 <= rows,
            "{cols} x {rows}: {next:?}"
        );
        assert_ne!(next, (cols, rows), "{cols} x {rows} does not fit but held");
        let shrunk = settle_from(&geometry, next, [false, true], false)
            .unwrap_or_else(|size| panic!("did not settle, last {size:?}"));
        assert_filled(&geometry, &shrunk, false);
    }
}

#[test]
fn herdr1_split_size_holds_when_a_pane_enters_the_alternate_screen() {
    let geometry = ipad(1240.);
    let surface = settle(&geometry, [false, false], false);
    let size = (surface.frame.width, surface.frame.height);
    for alternate in [[true, false], [false, true], [true, true]] {
        let entered = herdr1_split(size.0, size.1, alternate, false);
        assert_eq!(next_size(&geometry, &entered), size, "{alternate:?}");
    }
}

#[test]
fn borderless_split_keeps_its_column_budget() {
    // `ui.pane_borders = "off"` and no scrollbar: the panes are the terminals.
    let geometry = ipad(1240.);
    let mut surface = herdr1_split(139, 49, [true, true], false);
    for pane in &mut surface.panes {
        pane.inner_rect = pane.rect;
    }
    let panes = geometry.framed_panes(&surface);
    let viewport = geometry.viewport_size(&panes);
    let area = geometry.pane_area().size;
    let tightest = panes
        .iter()
        .map(|pane| area.width * (pane.room.size.width / pane.slot.size.width))
        .fold(area.width, |tightest, width| tightest.min(width));
    assert_eq!(viewport.width, tightest - px(CELL_WIDTH));
    // No split divides the rows, so no rounding row is held back.
    let rows = area.height - px(INNER_TOP + INNER_SIDE + 2. * metric::LINE);
    assert!((f32::from(viewport.height - rows)).abs() < 0.01);
}
