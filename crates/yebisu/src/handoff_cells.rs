// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/terminal/selection.rs:291-332 at 398c132a06723a0cc1494842a34666d173ec0200
use sheprd_client::protocol::CellData;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The bytes of a cell's symbol its width is read from. Terminal content is
/// untrusted and a symbol may be far longer than any grapheme, while what
/// follows a grapheme's first characters only combines with them.
const MAX_WIDTH_BYTES: usize = 64;

/// The cells of one row that read as text, each with its column, counted from
/// the first cell, and the columns it spans. The wire skip flag is not a
/// wide-cell marker: continuation cells can be empty or ordinary blanks, so a
/// wide grapheme covers the cells its width takes, and the scan starts at the
/// row's first cell so a range starting on a continuation recognizes it too.
/// Text in a covered cell is its own, and so is a blank after a symbol wide
/// only as a sequence: a terminal may keep an emoji presentation sequence in
/// one column, and only an empty cell shows it took two.
pub(super) fn graphemes(cells: &[CellData]) -> impl Iterator<Item = (usize, usize, &CellData)> {
    let mut covered = 0;
    let mut wide_base = false;
    cells.iter().enumerate().filter_map(move |(column, cell)| {
        let symbol = cell.symbol.as_str();
        if cell.skip || (column < covered && (symbol.is_empty() || (wide_base && symbol == " "))) {
            return None;
        }
        let head = &symbol[..symbol.floor_char_boundary(MAX_WIDTH_BYTES)];
        let span = head.width().max(1).min(cells.len() - column);
        covered = column + span;
        wide_base = head
            .chars()
            .next()
            .and_then(UnicodeWidthChar::width)
            .is_some_and(|width| width > 1);
        Some((column, span, cell))
    })
}

/// Whether each of `cells`, the start of a row, only continues the wide
/// grapheme before it, by the reading of [`graphemes`].
pub(super) fn continuations(cells: &[CellData]) -> Vec<bool> {
    let mut continuing = vec![true; cells.len()];
    for (column, _, _) in graphemes(cells) {
        continuing[column] = false;
    }
    continuing
}
