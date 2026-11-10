//! One coordinate system for painting, candidate rectangles and hit testing.
use gpui::{Bounds, Pixels, Point};
use sheprd_client::protocol::PaneSurfaceFrame;

#[derive(Clone, Copy, Default)]
pub(crate) struct InputGeometry {
    pub grid: Bounds<Pixels>,
    pub cell_width: f32,
    pub cell_height: f32,
}
impl InputGeometry {
    pub fn cursor(&self, surface: Option<&PaneSurfaceFrame>) -> Bounds<Pixels> {
        crate::terminal::input_cursor_bounds(
            surface,
            self.grid.origin,
            self.cell_width,
            self.cell_height,
        )
    }
    pub fn area(&self, surface: Option<&PaneSurfaceFrame>) -> Bounds<Pixels> {
        crate::terminal::input_area(surface, self.grid, self.cell_width, self.cell_height)
    }
    pub fn contains(&self, point: Point<Pixels>) -> bool {
        self.grid.contains(&point)
    }
}
