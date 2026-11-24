//! Text a terminal may have wrapped across rows. Modified from the fork's
//! links/wrap.rs. The surface carries no soft-wrap flag, so a row may go on
//! in the row below only when its text reaches the pane's right edge, or
//! stops one cell short because the next row starts with a wide glyph that
//! did not fit (Herdr 1's and Herdr 2's terminals both leave a blank there),
//! and the row below has text in its first column. Such a reading is only a
//! candidate: a line that happened to end at the edge looks the same, so an
//! address read across rows, or one that meets the edge, is opened only once
//! Herdr says where it ends (`super::resolve`). The fork also reads a
//! program's own indented wrap, for paths only; web addresses never go on
//! after an indent here either.
use super::{MAX_ROW_BYTES, continuations, shown};
use sheprd_client::protocol::{CellData, FrameData};
use std::ops::{Range, RangeInclusive};
use unicode_width::UnicodeWidthStr;

/// The most rows one wrapped address is read across, as in the fork, so a
/// screen of unbroken text costs a bounded amount per press.
pub(super) const MAX_WRAPPED_ROWS: u16 = 16;

/// Whether a cell reads as nothing, unless it continues a wide glyph, which
/// only the row around it can tell: see [`continuations`].
fn blank(cell: &CellData) -> bool {
    !cell.skip && shown(cell).chars().all(char::is_whitespace)
}

/// The cells of one pane in its frame, which a link never leaves.
pub(super) struct Area<'a> {
    frame: &'a FrameData,
    columns: Range<u16>,
    rows: Range<u16>,
}

/// The rows read around a press, and whether the text may go on above the
/// first, where `MAX_WRAPPED_ROWS` stopped the reading although the rows
/// still joined.
pub(super) struct Lines {
    pub(super) rows: RangeInclusive<u16>,
    pub(super) cut_above: bool,
}

impl<'a> Area<'a> {
    pub(super) fn new(frame: &'a FrameData, columns: Range<u16>, rows: Range<u16>) -> Self {
        Self {
            frame,
            columns,
            rows,
        }
    }

    /// The area's cells in frame row `row`, if it has that row.
    fn cells(&self, row: u16) -> Option<&'a [CellData]> {
        if !self.rows.contains(&row) {
            return None;
        }
        let offset = usize::from(row) * usize::from(self.frame.width);
        self.frame
            .cells
            .get(offset + usize::from(self.columns.start)..offset + usize::from(self.columns.end))
    }

    /// Whether text reaches the last cell of `row`. A continuation there
    /// follows its glyph, so the last two cells are enough.
    fn fills(&self, row: u16) -> bool {
        self.cells(row).is_some_and(|cells| {
            let end = cells.get(cells.len().saturating_sub(2)..).unwrap_or(cells);
            cells.last().is_some_and(|cell| !blank(cell))
                || continuations(end).last() == Some(&true)
        })
    }

    /// Whether `row` has text in its first cell.
    fn starts(&self, row: u16) -> bool {
        self.cells(row)
            .and_then(<[CellData]>::first)
            .is_some_and(|cell| !blank(cell))
    }

    /// Whether `row` ends in the blank a terminal leaves when the wide glyph
    /// starting the row below did not fit in its last cell: text reaches the
    /// cell before it, and the row below starts with a wide glyph.
    fn spacer(&self, row: u16) -> bool {
        let Some(cells) = self.cells(row) else {
            return false;
        };
        let [.., before, last] = cells else {
            return false;
        };
        let wide_below = row
            .checked_add(1)
            .and_then(|below| self.cells(below))
            .and_then(<[CellData]>::first)
            .is_some_and(|cell| shown(cell).width() > 1);
        blank(last) && !blank(before) && wide_below
    }

    /// Whether the text of row `above` may go on in the row below it.
    fn continues(&self, above: u16) -> bool {
        above
            .checked_add(1)
            .is_some_and(|below| (self.fills(above) || self.spacer(above)) && self.starts(below))
    }

    /// Whether text reaching the end of row `last` certainly ends there: the
    /// row below is in the area and starts with a blank, where a wrapped
    /// line would go on with its next character.
    pub(super) fn ends_line(&self, last: u16) -> bool {
        last.checked_add(1)
            .is_some_and(|below| self.cells(below).is_some() && !self.starts(below))
    }

    /// The rows the word at cell `index` of `row` may span, read as one
    /// line: up while the word starts its row under a row that goes on into
    /// it, down while it runs into the right edge of a row that goes on.
    pub(super) fn around(&self, row: u16, index: usize) -> Lines {
        let Some(cells) = self.cells(row) else {
            return Lines {
                rows: row..=row,
                cut_above: false,
            };
        };
        let continuing = continuations(cells);
        // A wrapped wide glyph's blank at the row's end breaks no word.
        let spacer = self.spacer(row);
        let breaks: Vec<bool> = cells
            .iter()
            .zip(&continuing)
            .enumerate()
            .map(|(index, (cell, &continuing))| {
                !continuing && blank(cell) && !(spacer && index + 1 == cells.len())
            })
            .collect();
        let starts_row = !breaks[..=index.min(breaks.len().saturating_sub(1))].contains(&true);
        let ends_row = !breaks.get(index..).unwrap_or_default().contains(&true);
        let mut first = row;
        let mut cut_above = false;
        if starts_row {
            while first > self.rows.start && self.continues(first - 1) {
                if row - first + 1 >= MAX_WRAPPED_ROWS {
                    cut_above = true;
                    break;
                }
                first -= 1;
            }
        }
        let mut last = row;
        if ends_row {
            // Text cut off below runs to the end of what is read, which
            // `found.open` already refuses.
            while last + 1 < self.rows.end
                && last - first + 1 < MAX_WRAPPED_ROWS
                && self.continues(last)
            {
                last += 1;
            }
        }
        Lines {
            rows: first..=last,
            cut_above,
        }
    }

    /// The text of `rows`, bounded like a single row's; `None` past that
    /// bound or the area.
    pub(super) fn read(&self, rows: RangeInclusive<u16>) -> Option<Run> {
        let first = *rows.start();
        let rows = rows
            .map(|row| self.cells(row))
            .collect::<Option<Vec<_>>>()?;
        let continuing: Vec<_> = rows.iter().map(|cells| continuations(cells)).collect();
        let mut text = String::new();
        let mut starts = Vec::with_capacity(rows.iter().map(|cells| cells.len()).sum());
        let spacers: Vec<bool> = (first..)
            .take(rows.len())
            .enumerate()
            .map(|(index, row)| index + 1 < rows.len() && self.spacer(row))
            .collect();
        for ((cells, continuing), &spacer) in rows.iter().zip(&continuing).zip(&spacers) {
            for (index, cell) in cells.iter().enumerate() {
                starts.push(text.len());
                // A wide glyph's blank left at a wrapped row's end is no text.
                if continuing[index] || (spacer && index + 1 == cells.len()) {
                    continue;
                }
                let symbol = shown(cell);
                if text.len() + symbol.len() > MAX_ROW_BYTES {
                    return None;
                }
                text.push_str(symbol);
            }
        }
        Some(Run {
            columns: self.columns.clone(),
            first,
            continuing,
            starts,
            text,
        })
    }
}

/// Consecutive rows of one area read as one text, and where each cell's text
/// starts in it, so a link found in the text maps back to its cells.
pub(super) struct Run {
    columns: Range<u16>,
    first: u16,
    /// Whether each cell of each row only continues a wide glyph.
    continuing: Vec<Vec<bool>>,
    /// The byte where each cell's text starts, row after row.
    starts: Vec<usize>,
    pub(super) text: String,
}

impl Run {
    /// Where the text of cell `index` of frame row `row` starts.
    pub(super) fn start_of(&self, row: u16, index: usize) -> Option<usize> {
        let width = usize::from(self.columns.end - self.columns.start);
        let offset = usize::from(row.checked_sub(self.first)?) * width;
        self.starts.get(offset + index).copied()
    }

    /// The frame columns the text in `range` covers on each row, top to
    /// bottom, leaving out rows it does not reach.
    pub(super) fn segments(&self, range: &Range<usize>) -> Vec<(u16, Range<u16>)> {
        let width = usize::from(self.columns.end - self.columns.start);
        // Cell indexes fit the row, whose width is a `u16`.
        let columns = |cells: Range<usize>| {
            self.columns.start + cells.start as u16..self.columns.start + cells.end as u16
        };
        (self.first..=u16::MAX)
            .zip(&self.continuing)
            .zip(self.starts.chunks(width.max(1)))
            .filter_map(|((row, continuing), starts)| {
                let cells = continuing.len();
                let on = |i: &usize| !continuing[*i] && range.contains(&starts[*i]);
                let first = (0..cells).find(on)?;
                let last = (0..cells).rev().find(on)?;
                // A wide character's continuation cells belong to the link too.
                let end = (last + 1..cells).find(|&i| !continuing[i]).unwrap_or(cells);
                Some((row, columns(first..end)))
            })
            .collect()
    }
}
