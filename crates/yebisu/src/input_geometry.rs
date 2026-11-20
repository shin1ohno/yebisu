//! One coordinate system for painting, candidate rectangles and hit testing.
use gpui::{Bounds, Pixels, Point, Size, point, px, size};
use sheprd_client::protocol::{PaneSurfaceFrame, PaneSurfacePane, SurfaceRect};

use crate::{shell::style::metric, terminal};

mod pane_layout;
pub(crate) use pane_layout::PanePlacement;

#[derive(Clone, Copy, Default)]
pub(crate) struct InputGeometry {
    pub grid: Bounds<Pixels>,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl InputGeometry {
    /// Selection.dc.html:77 and Rail.dc.html:86.
    const TERMINAL_PADDING_X: f32 = 14.;
    const TERMINAL_PADDING_Y: f32 = 12.;

    pub fn terminal_grid(&self) -> Bounds<Pixels> {
        inset(
            self.grid,
            Self::TERMINAL_PADDING_X,
            Self::TERMINAL_PADDING_Y,
            Self::TERMINAL_PADDING_X,
            Self::TERMINAL_PADDING_Y,
        )
    }

    pub fn cursor(&self, surface: Option<&PaneSurfaceFrame>) -> Bounds<Pixels> {
        if let Some(surface) = surface.filter(|s| s.popup.is_none())
            && let Some(pane) = self.framed_panes(surface).into_iter().find(|placement| {
                surface
                    .frame
                    .cursor
                    .as_ref()
                    .map_or(placement.pane.focused, |cursor| {
                        terminal::in_rect(placement.pane.inner_rect, cursor.x, cursor.y)
                    })
            })
        {
            return terminal::input_cursor_bounds(
                Some(surface),
                if surface.frame.cursor.is_some() {
                    pane.frame_origin
                } else {
                    pane.body.origin
                },
                self.cell_width,
                self.cell_height,
            );
        }
        if let Some(popup) = surface.and_then(|s| s.popup.as_ref()) {
            let origin = self.area(surface).origin;
            let origin = popup.frame.cursor.as_ref().map_or(origin, |cursor| {
                origin + terminal::cursor_offset(cursor, self.cell_width, self.cell_height)
            });
            return Bounds::new(origin, size(px(self.cell_width), px(self.cell_height)));
        }
        terminal::input_cursor_bounds(
            surface,
            self.terminal_grid().origin,
            self.cell_width,
            self.cell_height,
        )
    }

    pub fn area(&self, surface: Option<&PaneSurfaceFrame>) -> Bounds<Pixels> {
        let grid = self.terminal_grid();
        let Some(surface) = surface else { return grid };
        let panes = self.framed_panes(surface);
        if let Some(popup) = &surface.popup
            && !panes.is_empty()
        {
            let popup_size = self.cell_size(popup.frame.width, popup.frame.height);
            return Bounds::new(
                point(
                    grid.left() + (grid.size.width - popup_size.width).max(px(0.)) / 2.,
                    grid.top() + (grid.size.height - popup_size.height).max(px(0.)) / 2.,
                ),
                popup_size,
            );
        }
        if let Some(placement) = panes.iter().find(|placement| placement.pane.focused) {
            return placement.body;
        }
        terminal::input_area(Some(surface), grid, self.cell_width, self.cell_height)
    }

    pub fn cell_at(
        &self,
        surface: &PaneSurfaceFrame,
        position: Point<Pixels>,
        anchor: Option<(u16, u16)>,
    ) -> Option<(u16, u16)> {
        if let Some(popup) = &surface.popup {
            let area = self.area(Some(surface));
            if anchor.is_none() && !area.contains(&position) {
                return None;
            }
            return self.cell_in(position - area.origin, whole(&popup.frame));
        }
        let panes = self.framed_panes(surface);
        if !panes.is_empty() {
            let placement = panes.iter().find(|placement| {
                anchor.map_or_else(
                    || placement.body.contains(&position),
                    |(x, y)| terminal::in_rect(placement.pane.inner_rect, x, y),
                )
            })?;
            let rect = placement.pane.inner_rect;
            let body = placement.body;
            return self.cell_in(
                point(
                    position.x.clamp(body.left(), body.right()) - body.left(),
                    position.y.clamp(body.top(), body.bottom()) - body.top(),
                ),
                rect,
            );
        }
        let grid = self.terminal_grid();
        if anchor.is_none() && !grid.contains(&position) {
            return None;
        }
        self.cell_in(position - grid.origin, whole(&surface.frame))
    }

    fn cell_in(&self, relative: Point<Pixels>, rect: SurfaceRect) -> Option<(u16, u16)> {
        // Reversing a painted coordinate can lose a fraction of a point of
        // precision after adding and subtracting the translated grid origin.
        let cell = |offset: Pixels, extent: f32| {
            (f32::from(offset) / extent + 0.0001).floor().max(0.) as u16
        };
        let x = cell(relative.x, self.cell_width);
        let y = cell(relative.y, self.cell_height);
        Some((
            rect.x.saturating_add(x.min(rect.width.checked_sub(1)?)),
            rect.y.saturating_add(y.min(rect.height.checked_sub(1)?)),
        ))
    }

    pub fn contains(&self, point: Point<Pixels>) -> bool {
        self.grid.contains(&point)
    }

    fn cell_size(&self, width: u16, height: u16) -> Size<Pixels> {
        size(
            px(f32::from(width) * self.cell_width),
            px(f32::from(height) * self.cell_height),
        )
    }
}

fn whole(frame: &sheprd_client::protocol::FrameData) -> SurfaceRect {
    SurfaceRect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    }
}

fn inset(bounds: Bounds<Pixels>, left: f32, top: f32, right: f32, bottom: f32) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin + point(px(left), px(top)),
        size(
            (bounds.size.width - px(left + right)).max(px(0.)),
            (bounds.size.height - px(top + bottom)).max(px(0.)),
        ),
    )
}
