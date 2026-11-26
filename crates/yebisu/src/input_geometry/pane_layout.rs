use super::*;

// Main.dc.html:161-162,173 and Herdr2.dc.html:95-96,104 for the outer
// insets. The inner ones follow the owner's choice of 2026-11-26 (task
// 87ef7835b9ab449a, Q3 B): 8 pt, the gap between two panes, so the border
// sits evenly between the text and the next pane; 10 pt at the top, below
// the name on the border. The design had 12 and 14.
const OUTER_SIDE: f32 = 8.;
const OUTER_TOP: f32 = 10.;
const INNER_SIDE: f32 = metric::INLINE_GAP;
const INNER_TOP: f32 = 10.;
const HALF_GAP: f32 = metric::INLINE_GAP / 2.;
/// A slot this close to the pane area's extent spans the whole axis.
const SPAN_TOLERANCE: f32 = 0.5;

#[derive(Clone, Copy)]
pub(crate) struct PanePlacement<'a> {
    pub pane: &'a PaneSurfacePane,
    pub outline: Bounds<Pixels>,
    /// Where the pane's cells are drawn and hit: `room` less what whole
    /// cells cannot fill, split evenly between both sides (`centred`).
    pub body: Bounds<Pixels>,
    /// The outline less its border and inner padding: the space the resize
    /// budget offers the daemon (`viewport_size`).
    room: Bounds<Pixels>,
    /// The origin in paint space of global cell (0, 0) when painting this pane.
    pub frame_origin: Point<Pixels>,
    slot: Bounds<Pixels>,
    /// The daemon frame's columns and rows.
    frame: (u16, u16),
}

impl InputGeometry {
    fn pane_area(&self) -> Bounds<Pixels> {
        inset(
            self.canvas_bounds(),
            OUTER_SIDE,
            OUTER_TOP,
            OUTER_SIDE,
            OUTER_SIDE,
        )
    }

    /// Project daemon cells into UIKit points. Split gutters in the daemon's
    /// frame can be zero or several cells depending on its configuration.
    /// Snap both sides of each gutter to its midpoint before opening an 8 pt gap.
    pub(crate) fn framed_panes<'a>(&self, surface: &'a PaneSurfaceFrame) -> Vec<PanePlacement<'a>> {
        let frame = whole(&surface.frame);
        let area = self.pane_area();
        if surface.panes.len() < 2 || area.size.width <= px(0.) || area.size.height <= px(0.) {
            return Vec::new();
        }
        let (horizontal, vertical) = (Axis::Horizontal, Axis::Vertical);
        surface
            .panes
            .iter()
            .map(|pane| {
                if !inside(pane.inner_rect, pane.rect) || !inside(pane.rect, frame) {
                    return None;
                }
                let x = horizontal.edges(pane.rect, &surface.panes, frame.width);
                let y = vertical.edges(pane.rect, &surface.panes, frame.height);
                let point_x =
                    |cell| area.left() + area.size.width * (cell / f32::from(frame.width));
                let point_y =
                    |cell| area.top() + area.size.height * (cell / f32::from(frame.height));
                let slot = Bounds::from_corners(
                    point(point_x(x.0), point_y(y.0)),
                    point(point_x(x.1), point_y(y.1)),
                );
                let gap = |edge, outer| if edge == outer { 0. } else { HALF_GAP };
                let outline = inset(
                    slot,
                    gap(x.0, 0.),
                    gap(y.0, 0.),
                    gap(x.1, f32::from(frame.width)),
                    gap(y.1, f32::from(frame.height)),
                );
                let side = INNER_SIDE + metric::LINE;
                let room = inset(outline, side, INNER_TOP + metric::LINE, side, side);
                if room.size.width <= px(0.) || room.size.height <= px(0.) {
                    return None;
                }
                let source = pane.inner_rect;
                let body = self.centred(room, painted_columns(pane), source.height);
                let frame_origin = body.origin
                    - point(
                        px(f32::from(source.x) * self.cell_width),
                        px(f32::from(source.y) * self.cell_height),
                    );
                Some(PanePlacement {
                    pane,
                    outline,
                    body,
                    room,
                    frame_origin,
                    slot,
                    frame: (frame.width, frame.height),
                })
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// The part of `room` that `columns` by `rows` cells take, centred: what
    /// whole cells cannot fill is split between both sides instead of all
    /// left after the last column and row. Cells that do not fit (before
    /// the daemon answers a resize) take all of `room`, as before.
    fn centred(&self, room: Bounds<Pixels>, columns: u16, rows: u16) -> Bounds<Pixels> {
        let free = |extent: Pixels, count: u16, cell: f32| {
            (extent - px(f32::from(count) * cell)).max(px(0.)) / 2.
        };
        let x = free(room.size.width, columns, self.cell_width);
        let y = free(room.size.height, rows, self.cell_height);
        Bounds::new(
            room.origin + point(x, y),
            size(room.size.width - x * 2., room.size.height - y * 2.),
        )
    }

    /// Column width required by the tightest pane, based on the split shares
    /// rather than the frame's current size. Keep this threshold when zoomed.
    pub(crate) fn minimum_split_width(&self, surface: &PaneSurfaceFrame, text_width: f32) -> f32 {
        let span = f32::from(self.pane_area().size.width);
        self.framed_panes(surface)
            .iter()
            .map(|pane| {
                let chrome = f32::from(pane.slot.size.width - pane.room.size.width);
                let share = f32::from(pane.slot.size.width) / span;
                (text_width + chrome) / share + 2. * OUTER_SIDE
            })
            .fold(0., f32::max)
    }

    /// Budget for equal panes. This budget depends on points and font
    /// metrics only, so switching between an unframed and a framed surface
    /// cannot change whether two panes fit.
    pub(crate) fn equal_pane_viewport(&self, count: usize, vertical: bool) -> Size<Pixels> {
        let area = self.pane_area().size;
        let side = 2. * (INNER_SIDE + metric::LINE);
        let height = INNER_TOP + INNER_SIDE + 2. * metric::LINE;
        let gap = match count {
            0 | 1 => 0.,
            2 => HALF_GAP,
            _ => 2. * HALF_GAP,
        };
        let (x, y) = if vertical {
            (side, (height + gap) * count as f32)
        } else {
            ((side + gap) * count as f32, height)
        };
        size(
            (area.width - px(x + self.cell_width)).max(px(0.)),
            (area.height - px(y + self.cell_height)).max(px(0.)),
        )
    }

    /// Budget in cell space before sending a resize. Use the tightest pane in
    /// each axis and leave one cell for rounding at a daemon split boundary.
    /// An axis no split divides has no such boundary: every pane's slot is
    /// the whole axis at any size, so it needs no rounding cell.
    ///
    /// The slot spans the daemon's whole pane rectangle, border cells
    /// included, but the outline replaces those cells and the body holds only
    /// the cells inside them. Count them as body room, so the body does not
    /// keep their width empty after its last column and row.
    pub(crate) fn viewport_size(&self, panes: &[PanePlacement<'_>]) -> Size<Pixels> {
        if panes.is_empty() {
            return self.grid.size;
        }
        let area = self.pane_area().size;
        let mut available = area;
        let (mut split_x, mut split_y) = (false, false);
        for pane in panes {
            split_x |= pane.slot.size.width < area.width - px(SPAN_TOLERANCE);
            split_y |= pane.slot.size.height < area.height - px(SPAN_TOLERANCE);
            let (columns, rows) = border_cells(pane.pane);
            let width = pane.room.size.width + px(columns * self.cell_width);
            let height = pane.room.size.height + px(rows * self.cell_height);
            available.width = available
                .width
                .min(area.width * (width / pane.slot.size.width));
            available.height = available
                .height
                .min(area.height * (height / pane.slot.size.height));
        }
        let reserve = |split: bool, cell: f32| px(if split { cell } else { 0. });
        let budget = size(
            (available.width - reserve(split_x, self.cell_width)).max(px(0.)),
            (available.height - reserve(split_y, self.cell_height)).max(px(0.)),
        );
        self.keep_fitting_frame(panes, budget)
    }

    /// The budget scales each pane's chrome by the pane's share of the
    /// frame, and that share is the daemon's rounding of the split at the
    /// current size: an even split of an odd frame leaves the narrow pane
    /// under half. At 1220 pt the budget for 142 columns offers 143 and the
    /// budget for 143 offers 142, so the size never settles (#199). The
    /// rounding cell held back is for a size the daemon has not laid out yet;
    /// a frame whose cells already fit their bodies needs none. In each axis
    /// where they all fit, never offer less than the frame.
    fn keep_fitting_frame(
        &self,
        panes: &[PanePlacement<'_>],
        budget: Size<Pixels>,
    ) -> Size<Pixels> {
        const TOLERANCE: f32 = 0.01;
        let Some(first) = panes.first() else {
            return budget;
        };
        let (columns, rows) = first.frame;
        let fits_x = panes.iter().all(|pane| {
            f32::from(painted_columns(pane.pane)) * self.cell_width
                <= f32::from(pane.room.size.width) + TOLERANCE
        });
        let fits_y = panes.iter().all(|pane| {
            f32::from(pane.pane.inner_rect.height) * self.cell_height
                <= f32::from(pane.room.size.height) + TOLERANCE
        });
        // Half a cell past the frame's last column or row, so the floor in
        // `terminal::viewport` lands on the frame and not one short of it.
        let keep = |fits: bool, budget: Pixels, count: u16, cell: f32| {
            if fits {
                budget.max(px((f32::from(count) + 0.5) * cell))
            } else {
                budget
            }
        };
        size(
            keep(fits_x, budget.width, columns, self.cell_width),
            keep(fits_y, budget.height, rows, self.cell_height),
        )
    }
}

/// Border columns and rows the daemon keeps around a pane's terminal. On the
/// primary screen Herdr 1 also keeps the pane's last inner column for its
/// scrollbar, blank while there is no scrollback; that column is painted, so
/// it is not a border. Leaving it out keeps the count the same when a program
/// enters or leaves the alternate screen. When the scrollbar is turned off in
/// the daemon, this counts one column too few, which only leaves room.
fn border_cells(pane: &PaneSurfacePane) -> (f32, f32) {
    let (rect, inner) = (pane.rect, pane.inner_rect);
    let end = |start: u16, extent: u16| start.saturating_add(extent);
    let left = inner.x.saturating_sub(rect.x);
    let top = inner.y.saturating_sub(rect.y);
    let right = end(rect.x, rect.width).saturating_sub(end(inner.x, inner.width));
    let right = if pane.alternate_screen_active {
        right
    } else {
        right.saturating_sub(1)
    };
    let bottom = end(rect.y, rect.height).saturating_sub(end(inner.y, inner.height));
    (
        f32::from(left.saturating_add(right)),
        f32::from(top.saturating_add(bottom)),
    )
}

/// The columns drawn in a pane's body: its terminal, and on Herdr 1's
/// primary screen the scrollbar column after it, inside the daemon's border
/// (`border_cells`).
fn painted_columns(pane: &PaneSurfacePane) -> u16 {
    let (rect, inner) = (pane.rect, pane.inner_rect);
    let end = |start: u16, extent: u16| u32::from(start) + u32::from(extent);
    let scrollbar =
        !pane.alternate_screen_active && end(rect.x, rect.width) > end(inner.x, inner.width);
    inner.width.saturating_add(u16::from(scrollbar))
}

fn inside(inner: SurfaceRect, outer: SurfaceRect) -> bool {
    let ends_before = |a: u16, alen: u16, b: u16, blen: u16| {
        u32::from(a) + u32::from(alen) <= u32::from(b) + u32::from(blen)
    };
    inner.width > 0
        && inner.height > 0
        && inner.x >= outer.x
        && inner.y >= outer.y
        && ends_before(inner.x, inner.width, outer.x, outer.width)
        && ends_before(inner.y, inner.height, outer.y, outer.height)
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    fn range(self, rect: SurfaceRect) -> (u16, u16) {
        match self {
            Self::Horizontal => (rect.x, rect.x.saturating_add(rect.width)),
            Self::Vertical => (rect.y, rect.y.saturating_add(rect.height)),
        }
    }

    fn edges(self, pane: SurfaceRect, panes: &[PaneSurfacePane], extent: u16) -> (f32, f32) {
        let cross = match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        };
        let (start, end) = self.range(pane);
        let (cross_start, cross_end) = cross.range(pane);
        let mut before = None;
        let mut after = None;
        for pane in panes {
            let (next_start, next_end) = self.range(pane.rect);
            let (next_cross_start, next_cross_end) = cross.range(pane.rect);
            if next_cross_start >= cross_end || next_cross_end <= cross_start {
                continue;
            }
            if next_end <= start {
                before = Some(before.unwrap_or(0).max(next_end));
            }
            if next_start >= end {
                after = Some(after.unwrap_or(extent).min(next_start));
            }
        }
        let midpoint = |a, b| (f32::from(a) + f32::from(b)) / 2.;
        (
            before.map_or(0., |other| midpoint(other, start)),
            after.map_or(f32::from(extent), |other| midpoint(end, other)),
        )
    }
}

#[cfg(test)]
#[path = "pane_layout_tests.rs"]
mod tests;
