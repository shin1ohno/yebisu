// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/window/regions.rs at 398c132a06723a0cc1494842a34666d173ec0200
//! One frame painted with a pane's content drawn between rows
//! (`smooth_scroll`): the pane's chrome and the rest of the frame stay put,
//! the content inside the pane's inner rectangle moves by the slide's
//! offset, and the rows it uncovers come from earlier frames.

use super::{Ground, Highlight, Layer, Span, TerminalPainter};
use crate::upstream::smooth_scroll::Slide;
use gpui::{App, Bounds, ContentMask, Font, Pixels, Point, Size, Window, point, px, size};
use sheprd_client::protocol::{FrameData, PaneSurfacePane, SurfaceRect};
use std::ops::Range;

/// A slide with the painted cells split once, at the pane's inner edge.
pub(crate) struct Sliding<'a> {
    pub(crate) slide: &'a Slide,
    /// The cells inside the pane's inner rectangle, row by row.
    inside: Vec<Span>,
    outside: Vec<Span>,
}

impl<'a> Sliding<'a> {
    pub(crate) fn new(slide: &'a Slide, area: &[Span]) -> Self {
        let (inside, outside) = split(area, slide.rect);
        Self {
            slide,
            inside,
            outside,
        }
    }

    /// The cells inside the pane on `rows`.
    fn rows(&self, rows: Range<u16>) -> &[Span] {
        let start = self.inside.partition_point(|span| span.row < rows.start);
        let end = self.inside.partition_point(|span| span.row < rows.end);
        &self.inside[start..end]
    }
}

/// What one sliding paint draws: the frame, where its cell (0, 0) lands,
/// and the cells of it that a resting paint would draw.
pub(crate) struct SlidingFrame<'a> {
    pub(crate) frame: &'a FrameData,
    pub(crate) origin: Point<Pixels>,
    pub(crate) available: Option<Size<Pixels>>,
    pub(crate) area: &'a [Span],
    pub(crate) highlights: &'a [Highlight],
    pub(crate) panes: &'a [PaneSurfacePane],
}

impl TerminalPainter {
    /// Paints `shown` with `sliding`'s pane drawn `offset` rows below its
    /// grid position. Backgrounds paint at rest under the slide, so the
    /// sub-cell remainder past the grid's last row stays filled; text and
    /// decorations outside the pane paint at rest. Images never slide:
    /// a surface with placements does not start one (`slides_allowed`).
    pub(crate) fn paint_sliding(
        &mut self,
        shown: SlidingFrame<'_>,
        sliding: &Sliding<'_>,
        cell_width: f32,
        font: &Font,
        window: &mut Window,
        cx: &mut App,
    ) {
        let slide = sliding.slide;
        let rect = slide.rect;
        let cell_height = self.cell_height;
        // Whole device pixels, so every glyph rasterizes as it does at rest.
        let scale = window.scale_factor();
        let offset = (slide.offset * cell_height * scale).round() / scale;
        let mask = Bounds::new(
            shown.origin
                + point(
                    px(f32::from(rect.x) * cell_width),
                    px(f32::from(rect.y) * cell_height),
                ),
            size(
                px(f32::from(rect.width) * cell_width),
                px(f32::from(rect.height) * cell_height),
            ),
        );
        for layer in Layer::ALL {
            let still = match layer {
                Layer::Backgrounds => shown.area,
                Layer::Text | Layer::Decorations => &sliding.outside,
            };
            self.paint_frame_area(
                shown.frame,
                shown.origin,
                shown.available,
                Ground::Opaque,
                cell_width,
                font,
                shown.highlights,
                shown.panes,
                Some(still),
                Some(layer),
                None,
                window,
                cx,
            );
            window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
                // Each earlier frame fills the edge rows the newer ones lack.
                let mut filled = 0;
                for (earlier, shift) in &slide.behind {
                    let rows = u16::try_from(shift.abs_diff(filled))
                        .unwrap_or(rect.height)
                        .min(rect.height);
                    filled = *shift;
                    let bottom = rect.y.saturating_add(rect.height);
                    let band = match *shift > 0 {
                        true => bottom - rows..bottom,
                        false => rect.y..rect.y + rows,
                    };
                    let y = offset + *shift as f32 * cell_height;
                    self.paint_frame_area(
                        &earlier.frame,
                        shown.origin + point(px(0.), px(y)),
                        None,
                        Ground::Opaque,
                        cell_width,
                        font,
                        &[],
                        &[],
                        Some(sliding.rows(band)),
                        Some(layer),
                        None,
                        window,
                        cx,
                    );
                }
                self.paint_frame_area(
                    shown.frame,
                    shown.origin + point(px(0.), px(offset)),
                    None,
                    Ground::Opaque,
                    cell_width,
                    font,
                    shown.highlights,
                    shown.panes,
                    Some(&sliding.inside),
                    Some(layer),
                    None,
                    window,
                    cx,
                );
            });
        }
    }
}

/// The parts of `area` inside `rect`, and the parts outside it.
pub(crate) fn split(area: &[Span], rect: SurfaceRect) -> (Vec<Span>, Vec<Span>) {
    let rows = rect.y..rect.y.saturating_add(rect.height);
    let columns = rect.x..rect.x.saturating_add(rect.width);
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    for span in area {
        let start = span.columns.start.max(columns.start);
        let end = span.columns.end.min(columns.end);
        if !rows.contains(&span.row) || start >= end {
            outside.push(span.clone());
            continue;
        }
        inside.push(Span {
            row: span.row,
            columns: start..end,
        });
        for columns in [span.columns.start..start, end..span.columns.end] {
            if !columns.is_empty() {
                outside.push(Span {
                    row: span.row,
                    columns,
                });
            }
        }
    }
    (inside, outside)
}
