// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/terminal_painter/tests.rs at 398c132a06723a0cc1494842a34666d173ec0200
use super::*;
use core::prelude::v1::test;

mod backdrop;
mod cells;
mod composition;
mod diagnostics;
mod font_weight;
mod glyph_cache;
mod grid_edges;
// Paint counts are recorded only with the integration-test feature.
mod link_underline;
mod placed_images;
mod prompt_geometry;

fn cell(symbol: &str) -> CellData {
    CellData {
        symbol: symbol.into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    }
}
