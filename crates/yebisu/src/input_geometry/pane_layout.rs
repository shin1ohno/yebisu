use super::*;

// Main.dc.html:161-162,173 and Herdr2.dc.html:94-95,103.
const OUTER_SIDE: f32 = 8.;
const OUTER_TOP: f32 = 10.;
const INNER_SIDE: f32 = 12.;
const INNER_TOP: f32 = 14.;
const HALF_GAP: f32 = metric::INLINE_GAP / 2.;

#[derive(Clone, Copy)]
pub(crate) struct PanePlacement<'a> {
    pub pane: &'a PaneSurfacePane,
    pub outline: Bounds<Pixels>,
    pub body: Bounds<Pixels>,
    /// The origin in paint space of global cell (0, 0) when painting this pane.
    pub frame_origin: Point<Pixels>,
    slot: Bounds<Pixels>,
}

impl InputGeometry {
    fn pane_area(&self) -> Bounds<Pixels> {
        inset(self.grid, OUTER_SIDE, OUTER_TOP, OUTER_SIDE, OUTER_SIDE)
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
                let body = inset(outline, side, INNER_TOP + metric::LINE, side, side);
                if body.size.width <= px(0.) || body.size.height <= px(0.) {
                    return None;
                }
                let source = pane.inner_rect;
                let frame_origin = body.origin
                    - point(
                        px(f32::from(source.x) * self.cell_width),
                        px(f32::from(source.y) * self.cell_height),
                    );
                Some(PanePlacement {
                    pane,
                    outline,
                    body,
                    frame_origin,
                    slot,
                })
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// Herdr 2 stacks equal panes. This budget depends on points and font
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
    pub(crate) fn viewport_size(&self, panes: &[PanePlacement<'_>]) -> Size<Pixels> {
        if panes.is_empty() {
            return self.terminal_grid().size;
        }
        let area = self.pane_area().size;
        let mut available = area;
        for pane in panes {
            available.width = available
                .width
                .min(area.width * (pane.body.size.width / pane.slot.size.width));
            available.height = available
                .height
                .min(area.height * (pane.body.size.height / pane.slot.size.height));
        }
        size(
            (available.width - px(self.cell_width)).max(px(0.)),
            (available.height - px(self.cell_height)).max(px(0.)),
        )
    }
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
