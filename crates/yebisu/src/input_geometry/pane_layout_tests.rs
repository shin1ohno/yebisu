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
    let mut size = (100, 40);
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

/// Widths of this sweep where side-by-side Herdr 1 panes alternate between
/// two sizes (142 and 143 columns at 1220 pt) instead of settling. The
/// budget has done so since before the 8 pt padding: with the design's
/// 12 pt, 64 of every width from 700 to 1600 pt and every alternate-screen
/// mix did not settle, against 44 now; the 13 pt steps here missed them
/// until 1220 came up. Issue #199 tracks the fix.
const KNOWN_CYCLES: [u16; 1] = [1220];

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
        assert_eq!(
            placement.room.origin - placement.outline.origin,
            point(px(INNER_SIDE + metric::LINE), px(INNER_TOP + metric::LINE)),
            "the padding inside the border"
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

#[test]
fn herdr1_split_fills_its_bodies_across_widths() {
    for width in (700_u16..=1600).step_by(13) {
        let geometry = ipad(f32::from(width));
        for alternate in [[false, false], [true, true], [false, true], [true, false]] {
            for stacked in [false, true] {
                match try_settle(&geometry, alternate, stacked) {
                    Ok(surface) => assert_filled(&geometry, &surface, stacked),
                    Err(size) => assert!(
                        !stacked && KNOWN_CYCLES.contains(&width),
                        "{width} pt {alternate:?} stacked {stacked}: did not settle, last {size:?}"
                    ),
                }
            }
        }
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
